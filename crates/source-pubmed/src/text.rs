//! The two text shapes the DTD produces.

use std::borrow::Cow;

/// Decoded `#PCDATA` content: character and entity references resolved.
/// Borrowed from the input unless a reference had to be resolved. Not
/// trimmed and possibly empty — the serializer's whitespace and empty
/// elements are data to the normalizer, not noise to the parser.
pub type Text<'a> = Cow<'a, str>;

/// The inner markup of a mixed-content element — `(%text; | mml:math)*`,
/// where `%text;` is `#PCDATA | b | i | sup | sub | u` — exactly as
/// serialized: references undecoded, inline elements and `MathML` intact.
/// Always borrowed. What to do with `<sup>` or `<mml:math>` is a
/// normalization decision; this type refuses to make it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawXml<'a>(&'a str);

impl<'a> RawXml<'a> {
    /// Wraps a raw inner-markup span.
    #[must_use]
    pub const fn new(raw: &'a str) -> Self {
        Self(raw)
    }

    /// The span, exactly as it appeared between the element's tags.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }

    /// True for `<Elem></Elem>` and `<Elem/>`.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0.is_empty()
    }
}
