//! Text borrowed from a record's frame (plan §5, rule 1).
//!
//! serde's derive borrows a bare `Cow<str>` field marked `#[serde(borrow)]`
//! but copies the string when the `Cow` sits inside an `Option` or a list:
//! `Cow`'s own `Deserialize` always produces the owned form. Since most of
//! a record's strings are optional or listed, that was a `String` per field
//! per record. [`Text`] borrows wherever it appears — a JSON string without
//! escapes costs nothing — and owns only what had to be unescaped.

use std::borrow::{Borrow, Cow};
use std::fmt;
use std::ops::Deref;

use serde::de::{Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};

/// A string from the raw record: borrowed from the frame when the
/// serializer allowed, owned when it had to be unescaped.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Text<'a>(Cow<'a, str>);

impl<'a> Text<'a> {
    /// Borrowed text.
    #[must_use]
    pub const fn borrowed(s: &'a str) -> Self {
        Self(Cow::Borrowed(s))
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the text still borrows from the frame: no allocation was
    /// made for it.
    #[must_use]
    pub const fn is_borrowed(&self) -> bool {
        matches!(self.0, Cow::Borrowed(_))
    }

    /// The text, owned.
    #[must_use]
    pub fn into_owned(self) -> String {
        self.0.into_owned()
    }

    /// The underlying `Cow`.
    #[must_use]
    pub fn into_cow(self) -> Cow<'a, str> {
        self.0
    }
}

impl Deref for Text<'_> {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Text<'_> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Text<'_> {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl<'a> From<&'a str> for Text<'a> {
    fn from(s: &'a str) -> Self {
        Self(Cow::Borrowed(s))
    }
}

impl From<String> for Text<'_> {
    fn from(s: String) -> Self {
        Self(Cow::Owned(s))
    }
}

impl<'a> From<Cow<'a, str>> for Text<'a> {
    fn from(s: Cow<'a, str>) -> Self {
        Self(s)
    }
}

impl PartialEq<str> for Text<'_> {
    fn eq(&self, other: &str) -> bool {
        *self.0 == *other
    }
}

impl PartialEq<&str> for Text<'_> {
    fn eq(&self, other: &&str) -> bool {
        *self.0 == **other
    }
}

impl PartialEq<String> for Text<'_> {
    fn eq(&self, other: &String) -> bool {
        *self.0 == **other
    }
}

impl PartialEq<Text<'_>> for String {
    fn eq(&self, other: &Text<'_>) -> bool {
        **self == *other.0
    }
}

impl PartialEq<Text<'_>> for str {
    fn eq(&self, other: &Text<'_>) -> bool {
        *self == *other.0
    }
}

impl PartialEq<Text<'_>> for &str {
    fn eq(&self, other: &Text<'_>) -> bool {
        **self == *other.0
    }
}

impl fmt::Display for Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl Serialize for Text<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

struct TextVisitor;

impl<'de> Visitor<'de> for TextVisitor {
    type Value = Text<'de>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string")
    }

    fn visit_borrowed_str<E: serde::de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
        Ok(Text(Cow::Borrowed(v)))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(Text(Cow::Owned(v.to_owned())))
    }

    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(Text(Cow::Owned(v)))
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Text<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(TextVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::Text;

    #[derive(serde::Deserialize)]
    struct Record<'a> {
        #[serde(borrow)]
        plain: Text<'a>,
        #[serde(borrow)]
        optional: Option<Text<'a>>,
        #[serde(borrow)]
        listed: Vec<Text<'a>>,
        #[serde(borrow)]
        escaped: Option<Text<'a>>,
    }

    #[test]
    fn borrows_inside_options_and_lists_and_owns_only_unescaped_text() {
        let json = r#"{"plain":"a","optional":"b","listed":["c","d"],"escaped":"e\"f"}"#;
        let r: Record<'_> = serde_json::from_str(json).unwrap();
        assert!(r.plain.is_borrowed());
        assert!(r.optional.as_ref().unwrap().is_borrowed());
        assert!(r.listed.iter().all(Text::is_borrowed));
        let escaped = r.escaped.unwrap();
        assert!(!escaped.is_borrowed());
        assert_eq!(escaped, "e\"f");
        assert_eq!(&*r.plain, "a");
        assert_eq!(r.listed[1], "d");
        assert_eq!(serde_json::to_string(&r.plain).unwrap(), "\"a\"");
        assert_eq!(format!("{:?}", r.plain), "\"a\"");
        assert_eq!(Text::borrowed("x").into_owned(), "x");
    }
}
