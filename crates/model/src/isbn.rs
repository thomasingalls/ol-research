//! ISBNs.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// An ISBN-13 (thirteen digits, MOD 10 with weights 1 and 3) or ISBN-10
/// (nine digits and a MOD 11 check, `X` for ten), stored without the check
/// character. Crossref writes both unhyphenated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Isbn {
    /// The twelve-digit base of an ISBN-13.
    Thirteen(u64),
    /// The nine-digit base of an ISBN-10.
    Ten(u32),
}

/// Why a string is not an ISBN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Error {
    /// Not thirteen digits, nor nine digits and a check character.
    Shape,
    /// The check character does not match the digits.
    Checksum,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Shape => "not 13 digits or 10 characters",
            Self::Checksum => "checksum does not match",
        })
    }
}

impl std::error::Error for Error {}

fn check13(base: u64) -> u8 {
    let mut total = 0u64;
    let mut n = base;
    for i in 0..12 {
        total += (n % 10) * if i % 2 == 0 { 3 } else { 1 };
        n /= 10;
    }
    u8::try_from((10 - total % 10) % 10).unwrap_or(0)
}

fn check10(base: u32) -> u8 {
    let mut total = 0u32;
    let mut n = base;
    for weight in 2..=10 {
        total += (n % 10) * weight;
        n /= 10;
    }
    u8::try_from((11 - total % 11) % 11).unwrap_or(0)
}

impl Isbn {
    /// Parses `9781349545421` or `0412048418`.
    ///
    /// # Errors
    ///
    /// [`Error::Shape`] or [`Error::Checksum`].
    pub fn parse(s: &str) -> Result<Self, Error> {
        let b = s.as_bytes();
        let (digits, check) = match b.len() {
            13 => (&b[..12], b[12]),
            10 => (&b[..9], b[9]),
            _ => return Err(Error::Shape),
        };
        if !digits.iter().all(u8::is_ascii_digit) {
            return Err(Error::Shape);
        }
        let base: u64 = digits.iter().fold(0, |n, &c| n * 10 + u64::from(c - b'0'));
        let check = match check {
            b'X' if b.len() == 10 => 10,
            d if d.is_ascii_digit() => d - b'0',
            _ => return Err(Error::Shape),
        };
        if b.len() == 13 {
            if check != check13(base) {
                return Err(Error::Checksum);
            }
            Ok(Self::Thirteen(base))
        } else {
            let base = u32::try_from(base).map_err(|_| Error::Shape)?;
            if check != check10(base) {
                return Err(Error::Checksum);
            }
            Ok(Self::Ten(base))
        }
    }
}

impl fmt::Display for Isbn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Thirteen(base) => write!(f, "{base:012}{}", check13(base)),
            Self::Ten(base) => {
                write!(f, "{base:09}")?;
                match check10(base) {
                    10 => f.write_str("X"),
                    d => write!(f, "{d}"),
                }
            }
        }
    }
}

impl Serialize for Isbn {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let packed = match *self {
            Self::Thirteen(base) => (true, base),
            Self::Ten(base) => (false, u64::from(base)),
        };
        crate::human::serialize(self, packed, serializer)
    }
}

impl<'de> Deserialize<'de> for Isbn {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::human::deserialize(
            deserializer,
            "ISBN",
            |(thirteen, base): (bool, u64)| {
                if thirteen {
                    (base < 1_000_000_000_000).then_some(Self::Thirteen(base))
                } else {
                    u32::try_from(base)
                        .ok()
                        .filter(|b| *b < 1_000_000_000)
                        .map(Self::Ten)
                }
            },
            |s| Self::parse(s).ok(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Isbn};

    #[test]
    fn observed_forms() {
        for s in [
            "9781349545421",
            "9780230627574",
            "9783540359784",
            "0412048418",
            "1558602380",
            "9171468765",
        ] {
            assert_eq!(Isbn::parse(s).unwrap().to_string(), s, "{s}");
        }
    }

    #[test]
    fn rejections() {
        assert_eq!(Isbn::parse("9781349545422"), Err(Error::Checksum));
        assert_eq!(Isbn::parse("0412048419"), Err(Error::Checksum));
        assert_eq!(Isbn::parse("978-1-349-54542-1"), Err(Error::Shape));
        assert_eq!(Isbn::parse("978134954542X"), Err(Error::Shape));
        assert_eq!(Isbn::parse("155860238x"), Err(Error::Shape));
    }
}
