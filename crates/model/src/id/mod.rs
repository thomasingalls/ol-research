//! Native identifiers and their parsing rules.
//!
//! Every parser here is strict in the fail-fast sense (plan §6): it accepts
//! exactly the form a source's serializer was observed to emit, and nothing
//! else. A sign, a leading zero, surrounding whitespace or a lowercase prefix
//! is a difference worth stopping for, not one to paper over. That is why the
//! digit loop is hand-written: `str::parse::<u32>` accepts `+1` and `007`.
//!
//! Each type has one entry point per observed form, so a source that starts
//! emitting the other form fails at the source that changed.

mod openalex;
mod pmid;

use std::fmt;

pub use openalex::OpenAlexId;
pub use pmid::Pmid;

/// Why an identifier failed to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseError {
    /// Nothing to parse (after any prefix).
    Empty,
    /// The input did not start with the required literal prefix.
    BadPrefix,
    /// A byte that is not an ASCII digit, at this offset into the input.
    NotDigit {
        /// Byte offset into the whole input, prefix included.
        offset: usize,
    },
    /// A number written with a leading zero.
    LeadingZero,
    /// Zero is not a valid identifier.
    Zero,
    /// The number does not fit the identifier's width.
    Overflow,
}

impl ParseError {
    /// Moves a [`ParseError::NotDigit`] offset by `by` bytes, for callers
    /// that stripped a prefix before parsing the digits.
    const fn shift(self, by: usize) -> Self {
        match self {
            Self::NotDigit { offset } => Self::NotDigit {
                offset: offset + by,
            },
            other => other,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty"),
            Self::BadPrefix => f.write_str("missing or wrong prefix"),
            Self::NotDigit { offset } => write!(f, "non-digit at byte {offset}"),
            Self::LeadingZero => f.write_str("leading zero"),
            Self::Zero => f.write_str("zero"),
            Self::Overflow => f.write_str("out of range"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Strips `prefix` from the front of `s`, or fails with
/// [`ParseError::BadPrefix`].
fn strip_literal<'a>(s: &'a str, prefix: &str) -> Result<&'a str, ParseError> {
    s.strip_prefix(prefix).ok_or(ParseError::BadPrefix)
}

/// Parses `s` as an unsigned decimal: ASCII digits only, no sign, no leading
/// zero, no surrounding whitespace, non-zero, at most `max`.
///
/// A non-digit byte is reported before the leading-zero rule, so `0x` is
/// [`ParseError::NotDigit`] at offset 1, not [`ParseError::LeadingZero`].
fn parse_digits(s: &str, max: u64) -> Result<u64, ParseError> {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return Err(ParseError::Empty);
    }
    let mut value: u64 = 0;
    for (offset, &byte) in bytes.iter().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(ParseError::NotDigit { offset });
        }
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(u64::from(byte - b'0')))
            .ok_or(ParseError::Overflow)?;
        if value > max {
            return Err(ParseError::Overflow);
        }
    }
    if bytes[0] == b'0' {
        return Err(if bytes.len() == 1 {
            ParseError::Zero
        } else {
            ParseError::LeadingZero
        });
    }
    Ok(value)
}

/// Deterministic xorshift64 stream for roundtrip tests; `seed` must be
/// non-zero.
#[cfg(test)]
pub(crate) fn pseudo_random(mut state: u64) -> impl Iterator<Item = u64> {
    std::iter::from_fn(move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        Some(state)
    })
}

#[cfg(test)]
mod tests {
    use super::{ParseError, parse_digits, pseudo_random};

    const U32: u64 = u32::MAX as u64;

    #[test]
    fn accepts_canonical_decimals() {
        assert_eq!(parse_digits("1", U32), Ok(1));
        assert_eq!(parse_digits("9", U32), Ok(9));
        assert_eq!(parse_digits("10", U32), Ok(10));
        assert_eq!(parse_digits("4294967295", U32), Ok(U32));
        assert_eq!(parse_digits("18446744073709551615", u64::MAX), Ok(u64::MAX));
    }

    #[test]
    fn rejects_empty_and_zero() {
        assert_eq!(parse_digits("", U32), Err(ParseError::Empty));
        assert_eq!(parse_digits("0", U32), Err(ParseError::Zero));
    }

    #[test]
    fn rejects_leading_zeros() {
        for s in ["00", "01", "007", "0000000000000000000000000"] {
            assert_eq!(parse_digits(s, U32), Err(ParseError::LeadingZero), "{s:?}");
        }
    }

    #[test]
    fn rejects_anything_str_parse_would_tolerate() {
        // `str::parse::<u32>` accepts the first two; we do not.
        let cases: &[(&str, usize)] = &[
            ("+1", 0),
            ("-1", 0),
            (" 1", 0),
            ("1 ", 1),
            ("1\n", 1),
            ("12a", 2),
            ("1.0", 1),
            ("1e3", 1),
            ("0x", 1),
            ("\u{661}", 0),  // Arabic-Indic one: numeric, not an ASCII digit
            ("\u{ff11}", 0), // fullwidth one
        ];
        for &(s, offset) in cases {
            assert_eq!(
                parse_digits(s, U32),
                Err(ParseError::NotDigit { offset }),
                "{s:?}"
            );
        }
    }

    #[test]
    fn rejects_overflow_at_both_widths() {
        assert_eq!(parse_digits("4294967296", U32), Err(ParseError::Overflow));
        assert_eq!(
            parse_digits("18446744073709551616", u64::MAX),
            Err(ParseError::Overflow)
        );
        assert_eq!(
            parse_digits("99999999999999999999999", u64::MAX),
            Err(ParseError::Overflow)
        );
    }

    #[test]
    fn roundtrips_pseudo_random_values() {
        for v in pseudo_random(0x9E37_79B9_7F4A_7C15).take(10_000) {
            assert_eq!(parse_digits(&v.to_string(), u64::MAX), Ok(v));
            let narrow = u64::from(u32::try_from(v & U32).unwrap_or(1).max(1));
            assert_eq!(parse_digits(&narrow.to_string(), U32), Ok(narrow));
        }
    }

    #[test]
    fn shift_moves_only_not_digit() {
        assert_eq!(
            ParseError::NotDigit { offset: 2 }.shift(5),
            ParseError::NotDigit { offset: 7 }
        );
        assert_eq!(ParseError::Overflow.shift(5), ParseError::Overflow);
    }
}
