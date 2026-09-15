//! MeSH unique identifiers.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A MeSH unique identifier, validated and borrowed: `D016428`,
/// `D000078182`, `Q000378`, `C000724324`.
///
/// One letter — `D` descriptor, `Q` qualifier, `C` supplementary concept —
/// followed by six or nine digits. NLM widened the number to nine digits
/// for newer records while keeping six for old ones, and the width is part
/// of the identifier (`D000078182` is not `D078182`), so the text is the
/// canonical form and this type keeps it rather than packing a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MeshUi<'a>(&'a str);

/// The record kind a MeSH UI names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    /// `D…`: a descriptor, including publication types.
    Descriptor,
    /// `Q…`: a qualifier.
    Qualifier,
    /// `C…`: a supplementary concept record.
    Supplementary,
}

/// Why a string is not a MeSH UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Error {
    /// Nothing to parse.
    Empty,
    /// The first byte is not `D`, `Q` or `C`.
    BadKind,
    /// Not six or nine digits after the letter.
    BadLength,
    /// A byte that is not an ASCII digit, at this offset.
    NotDigit {
        /// Byte offset into the whole input.
        offset: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty"),
            Self::BadKind => f.write_str("does not start with D, Q or C"),
            Self::BadLength => f.write_str("not 6 or 9 digits after the letter"),
            Self::NotDigit { offset } => write!(f, "non-digit at byte {offset}"),
        }
    }
}

impl std::error::Error for Error {}

impl<'a> MeshUi<'a> {
    /// Validates a UI.
    ///
    /// # Errors
    ///
    /// See [`Error`].
    pub fn parse(s: &'a str) -> Result<Self, Error> {
        let mut bytes = s.bytes();
        match bytes.next() {
            None => return Err(Error::Empty),
            Some(b'D' | b'Q' | b'C') => {}
            Some(_) => return Err(Error::BadKind),
        }
        let digits = s.len() - 1;
        if digits != 6 && digits != 9 {
            return Err(Error::BadLength);
        }
        if let Some(bad) = bytes.position(|b| !b.is_ascii_digit()) {
            return Err(Error::NotDigit { offset: bad + 1 });
        }
        Ok(Self(s))
    }

    /// The identifier text.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }

    /// Which kind of record it names.
    #[must_use]
    pub fn kind(self) -> Kind {
        match self.0.as_bytes()[0] {
            b'D' => Kind::Descriptor,
            b'Q' => Kind::Qualifier,
            _ => Kind::Supplementary,
        }
    }
}

impl fmt::Display for MeshUi<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Kind, MeshUi};

    #[test]
    fn observed_forms() {
        assert_eq!(MeshUi::parse("D016428").unwrap().kind(), Kind::Descriptor);
        assert_eq!(MeshUi::parse("D000078182").unwrap().as_str(), "D000078182");
        assert_eq!(MeshUi::parse("Q000378").unwrap().kind(), Kind::Qualifier);
        assert_eq!(
            MeshUi::parse("C000724324").unwrap().kind(),
            Kind::Supplementary
        );
    }

    #[test]
    fn rejections() {
        assert_eq!(MeshUi::parse(""), Err(Error::Empty));
        assert_eq!(MeshUi::parse("X016428"), Err(Error::BadKind));
        assert_eq!(MeshUi::parse("D16428"), Err(Error::BadLength));
        assert_eq!(MeshUi::parse("D0164280"), Err(Error::BadLength));
        assert_eq!(MeshUi::parse("D01642x"), Err(Error::NotDigit { offset: 6 }));
        assert_eq!(MeshUi::parse("d016428"), Err(Error::BadKind));
    }
}
