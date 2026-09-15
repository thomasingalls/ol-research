//! Research Organization Registry identifiers.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A ROR id: `https://ror.org/` then a leading `0`, six characters of
/// Crockford base32 (lowercase, no `i`, `l`, `o` or `u`) and a two-digit
/// ISO 7064 MOD 97-10 checksum over the number those six characters encode.
/// Stored as that 30-bit number; the checksum is derived.
///
/// OpenAlex writes the URL form on every institution and most funders;
/// every one in the sample (5,795 institution ids, 2,571 funder ids) passed
/// the checksum. Crossref's affiliation ids carry the same form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ror(u32);

/// Why a string is not a ROR id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Error {
    /// Not the `https://ror.org/` prefix.
    BadPrefix,
    /// Not `0`, six lowercase Crockford base32 characters and two digits.
    Shape,
    /// The two digits do not match the base32 number.
    Checksum,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadPrefix => "missing or wrong URL prefix",
            Self::Shape => "not 0, six base32 characters and two digits",
            Self::Checksum => "checksum does not match",
        })
    }
}

impl std::error::Error for Error {}

/// Crockford's alphabet, lowercase as ROR writes it.
const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

fn digit(c: u8) -> Option<u32> {
    ALPHABET
        .iter()
        .position(|&a| a == c)
        .and_then(|p| u32::try_from(p).ok())
}

/// ISO 7064 MOD 97-10 over the number: `2..=98`.
fn checksum(n: u32) -> u8 {
    u8::try_from(98 - (u64::from(n) * 100) % 97).unwrap_or(0)
}

impl Ror {
    /// The prefix every source writes.
    pub const URL_PREFIX: &'static str = "https://ror.org/";

    /// Parses the bare form, `052gg0110`.
    ///
    /// # Errors
    ///
    /// [`Error::Shape`] or [`Error::Checksum`].
    pub fn from_bare(s: &str) -> Result<Self, Error> {
        let b = s.as_bytes();
        if b.len() != 9 || b[0] != b'0' {
            return Err(Error::Shape);
        }
        let mut n = 0u32;
        for &c in &b[1..7] {
            n = (n << 5) | digit(c).ok_or(Error::Shape)?;
        }
        if !b[7].is_ascii_digit() || !b[8].is_ascii_digit() {
            return Err(Error::Shape);
        }
        let check = (b[7] - b'0') * 10 + (b[8] - b'0');
        if check != checksum(n) {
            return Err(Error::Checksum);
        }
        Ok(Self(n))
    }

    /// Parses the URL form, `https://ror.org/052gg0110`.
    ///
    /// # Errors
    ///
    /// [`Error::BadPrefix`], then as [`Self::from_bare`].
    pub fn from_url(s: &str) -> Result<Self, Error> {
        let rest = s.strip_prefix(Self::URL_PREFIX).ok_or(Error::BadPrefix)?;
        Self::from_bare(rest)
    }

    /// The 30-bit number the six base32 characters encode.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Ror {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut chars = [b'0'; 7];
        let mut n = self.0;
        for slot in chars[1..].iter_mut().rev() {
            *slot = ALPHABET[(n & 31) as usize];
            n >>= 5;
        }
        f.write_str(std::str::from_utf8(&chars).unwrap_or(""))?;
        write!(f, "{:02}", checksum(self.0))
    }
}

impl Serialize for Ror {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::human::serialize(self, self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for Ror {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::human::deserialize(
            deserializer,
            "ROR",
            |n: u32| (n < 1 << 30).then_some(Self(n)),
            |s| Self::from_bare(s).ok(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Ror};

    #[test]
    fn observed_forms_roundtrip() {
        // Oxford, Chicago, and a funder, from the OpenAlex part file.
        for s in [
            "052gg0110",
            "024mw5h28",
            "01cwqze88",
            "02xankh89",
            "00k4n6c32",
        ] {
            let r = Ror::from_bare(s).unwrap();
            assert_eq!(r.to_string(), s);
            assert_eq!(
                Ror::from_url(&format!("{}{s}", Ror::URL_PREFIX)),
                Ok(r),
                "{s}"
            );
        }
    }

    #[test]
    fn rejections() {
        assert_eq!(Ror::from_bare("052gg0111"), Err(Error::Checksum));
        assert_eq!(Ror::from_bare("152gg0110"), Err(Error::Shape));
        assert_eq!(Ror::from_bare("052GG0110"), Err(Error::Shape), "uppercase");
        assert_eq!(Ror::from_bare("05igg0110"), Err(Error::Shape), "no i");
        assert_eq!(Ror::from_bare("052gg011"), Err(Error::Shape));
        assert_eq!(Ror::from_bare("052gg01a0"), Err(Error::Shape));
        assert_eq!(Ror::from_url("052gg0110"), Err(Error::BadPrefix));
        assert_eq!(
            Ror::from_url("http://ror.org/052gg0110"),
            Err(Error::BadPrefix)
        );
        assert_eq!(
            Ror::from_url("https://ror.org/052gg0110/"),
            Err(Error::Shape)
        );
    }

    #[test]
    fn number_and_width() {
        assert_eq!(Ror::from_bare("000000098").map(Ror::get), Ok(0));
        assert_eq!(Ror::from_bare("000000002"), Err(Error::Checksum));
        let max = Ror::from_bare(&format!(
            "0zzzzzz{:02}",
            98 - ((u64::from(u32::MAX >> 2) * 100) % 97)
        ))
        .unwrap();
        assert_eq!(max.get(), (1 << 30) - 1);
        assert_eq!(size_of::<Option<Ror>>(), 8);
    }
}
