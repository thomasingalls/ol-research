//! ISSNs.

use std::fmt;

/// An ISSN: `NNNN-NNNC`, the last character a MOD 11 check digit (`X` for
/// ten). Stored as the seven-digit number; the check digit is derived.
/// Crossref and PubMed both write the hyphenated form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Issn(u32);

/// Why a string is not an ISSN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    /// Not `NNNN-NNNC`.
    Shape,
    /// The check character does not match the digits.
    Checksum,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Shape => "not NNNN-NNNC",
            Self::Checksum => "checksum does not match",
        })
    }
}

impl std::error::Error for Error {}

/// MOD 11 with weights 8 down to 2: `0..=10`, ten meaning `X`.
fn checksum(base: u32) -> u8 {
    let mut total = 0;
    let mut n = base;
    for weight in 2..=8 {
        total += (n % 10) * weight;
        n /= 10;
    }
    u8::try_from((11 - total % 11) % 11).unwrap_or(0)
}

impl Issn {
    /// Parses `2168-8184`.
    ///
    /// # Errors
    ///
    /// [`Error::Shape`] or [`Error::Checksum`].
    pub fn parse(s: &str) -> Result<Self, Error> {
        let b = s.as_bytes();
        if b.len() != 9 || b[4] != b'-' {
            return Err(Error::Shape);
        }
        let mut base = 0u32;
        for &c in b[..4].iter().chain(&b[5..8]) {
            if !c.is_ascii_digit() {
                return Err(Error::Shape);
            }
            base = base * 10 + u32::from(c - b'0');
        }
        let check = match b[8] {
            b'X' => 10,
            d if d.is_ascii_digit() => d - b'0',
            _ => return Err(Error::Shape),
        };
        if check != checksum(base) {
            return Err(Error::Checksum);
        }
        Ok(Self(base))
    }

    /// The seven digits without the check character.
    #[must_use]
    pub const fn base(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Issn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{:07}", self.0);
        write!(f, "{}-{}", &s[..4], &s[4..7])?;
        match checksum(self.0) {
            10 => f.write_str("X"),
            d => write!(f, "{d}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Issn};

    #[test]
    fn observed_forms() {
        for s in [
            "2168-8184",
            "2167-8359",
            "2045-2322",
            "0004-637X",
            "1538-3598",
            "0098-7484",
        ] {
            assert_eq!(Issn::parse(s).unwrap().to_string(), s);
        }
    }

    #[test]
    fn rejections() {
        assert_eq!(Issn::parse("2168-8185"), Err(Error::Checksum));
        assert_eq!(Issn::parse("21688184"), Err(Error::Shape));
        assert_eq!(Issn::parse("2168-818"), Err(Error::Shape));
        assert_eq!(Issn::parse("0004-637x"), Err(Error::Shape));
        assert_eq!(Issn::parse(" 2168-8184"), Err(Error::Shape));
    }
}
