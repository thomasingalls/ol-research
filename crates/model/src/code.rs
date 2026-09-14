//! Short uppercase-letter codes: ISO 3166-1 alpha-2 countries and ISO 4217
//! currencies.
//!
//! By shape only, so far: exactly two or three uppercase ASCII letters,
//! which catches lowercase, whitespace, digits and the wrong width — every
//! code in the OpenAlex sample (4,878 authorship countries, 5,401
//! institution countries, 778 currencies) has that shape. Whether the
//! letters name a real country or currency waits for a vocabulary table
//! (plan §8); the types are sized so that adding one changes nothing else.

use std::fmt;

/// An ISO 3166-1 alpha-2 country code, `US`. Two bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CountryCode([u8; 2]);

/// An ISO 4217 currency code, `USD`. Three bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CurrencyCode([u8; 3]);

/// Why a string is not a code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    /// Not the code's width.
    Length,
    /// A byte that is not an uppercase ASCII letter, at this offset.
    NotUppercase {
        /// Byte offset into the input.
        offset: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length => f.write_str("wrong length"),
            Self::NotUppercase { offset } => {
                write!(f, "not an uppercase ASCII letter at byte {offset}")
            }
        }
    }
}

impl std::error::Error for Error {}

fn letters<const N: usize>(s: &str) -> Result<[u8; N], Error> {
    let bytes: [u8; N] = s.as_bytes().try_into().map_err(|_| Error::Length)?;
    if let Some(offset) = bytes.iter().position(|b| !b.is_ascii_uppercase()) {
        return Err(Error::NotUppercase { offset });
    }
    Ok(bytes)
}

impl CountryCode {
    /// Parses exactly two uppercase ASCII letters.
    ///
    /// # Errors
    ///
    /// See [`Error`].
    pub fn parse(s: &str) -> Result<Self, Error> {
        letters(s).map(Self)
    }

    /// The code.
    #[must_use]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("")
    }
}

impl CurrencyCode {
    /// Parses exactly three uppercase ASCII letters.
    ///
    /// # Errors
    ///
    /// See [`Error`].
    pub fn parse(s: &str) -> Result<Self, Error> {
        letters(s).map(Self)
    }

    /// The code.
    #[must_use]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("")
    }
}

impl fmt::Display for CountryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for CurrencyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{CountryCode, CurrencyCode, Error};

    #[test]
    fn observed_forms() {
        for s in ["US", "FR", "GB", "CN"] {
            assert_eq!(CountryCode::parse(s).unwrap().to_string(), s);
        }
        for s in ["USD", "EUR", "GBP", "CHF"] {
            assert_eq!(CurrencyCode::parse(s).unwrap().to_string(), s);
        }
        assert_eq!(size_of::<Option<CountryCode>>(), 3);
        assert_eq!(size_of::<CurrencyCode>(), 3);
    }

    #[test]
    fn rejections() {
        assert_eq!(CountryCode::parse("USA"), Err(Error::Length));
        assert_eq!(CountryCode::parse("U"), Err(Error::Length));
        assert_eq!(CountryCode::parse(""), Err(Error::Length));
        assert_eq!(
            CountryCode::parse("us"),
            Err(Error::NotUppercase { offset: 0 })
        );
        assert_eq!(
            CountryCode::parse("U1"),
            Err(Error::NotUppercase { offset: 1 })
        );
        assert_eq!(CurrencyCode::parse("US"), Err(Error::Length));
        assert_eq!(
            CurrencyCode::parse("Us$"),
            Err(Error::NotUppercase { offset: 1 })
        );
        assert_eq!(
            CountryCode::parse("ÉS"),
            Err(Error::Length),
            "two bytes of É"
        );
    }
}
