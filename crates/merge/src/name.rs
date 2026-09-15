//! Names: what the world calls a work (merge plan §2), and a record's
//! assertions of them (§3.1).

use std::fmt;

use ingest::Key;
use model::{Doi, OpenAlexId, PmcId, Pmid};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A name borrowed from a record: a namespace and a value. A DOI is
/// canonical text, lowercase and bare, as `model::Doi` holds it; the rest
/// are the model's packed ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Name<'a> {
    #[allow(missing_docs)]
    Doi(&'a str),
    #[allow(missing_docs)]
    Pmid(Pmid),
    #[allow(missing_docs)]
    Pmc(PmcId),
    #[allow(missing_docs)]
    OpenAlex(OpenAlexId),
}

impl<'a> Name<'a> {
    /// The namespace.
    #[must_use]
    pub const fn key(&self) -> Key {
        match self {
            Self::Doi(_) => Key::Doi,
            Self::Pmid(_) => Key::Pmid,
            Self::Pmc(_) => Key::Pmc,
            Self::OpenAlex(_) => Key::OpenAlex,
        }
    }

    /// The number of a numeric name; `None` for a DOI.
    #[must_use]
    pub const fn number(&self) -> Option<u64> {
        match self {
            Self::Doi(_) => None,
            Self::Pmid(p) => Some(p.get() as u64),
            Self::Pmc(p) => Some(p.get() as u64),
            Self::OpenAlex(w) => Some(w.get()),
        }
    }

    /// Parses the text form this type displays, the DOI's text borrowed
    /// from `text`, through the model's own parsers, so only a canonical
    /// name reads back.
    ///
    /// # Errors
    ///
    /// No namespace, an unknown one, or a value its parser refuses.
    pub fn parse(text: &'a str) -> Result<Self, ParseNameError> {
        let err = |reason: String| ParseNameError {
            text: text.to_owned(),
            reason,
        };
        let (key, value) = text
            .split_once(':')
            .ok_or_else(|| err("no namespace".to_owned()))?;
        let key = Key::parse(key).ok_or_else(|| err(format!("unknown namespace {key:?}")))?;
        Ok(match key {
            Key::Doi => {
                let doi = Doi::parse(value).map_err(|e| err(e.to_string()))?;
                if doi.as_str() != value {
                    return Err(err("not canonical".to_owned()));
                }
                Self::Doi(value)
            }
            Key::Pmid => Self::Pmid(Pmid::from_digits(value).map_err(|e| err(e.to_string()))?),
            Key::Pmc => Self::Pmc(PmcId::from_prefixed(value).map_err(|e| err(e.to_string()))?),
            Key::OpenAlex => {
                Self::OpenAlex(OpenAlexId::from_bare(value).map_err(|e| err(e.to_string()))?)
            }
        })
    }

    /// A copy that owns its text.
    #[must_use]
    pub fn to_owned(&self) -> OwnedName {
        match *self {
            Self::Doi(d) => OwnedName::Doi(d.into()),
            Self::Pmid(p) => OwnedName::Pmid(p),
            Self::Pmc(p) => OwnedName::Pmc(p),
            Self::OpenAlex(w) => OwnedName::OpenAlex(w),
        }
    }
}

/// `doi:10.1000/x`, `pmid:123`, `pmc:PMC123`, `openalex:W123`: the
/// namespace's [`Key`] name, a colon, the model's canonical text.
impl fmt::Display for Name<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.key())?;
        match self {
            Self::Doi(d) => f.write_str(d),
            Self::Pmid(p) => write!(f, "{p}"),
            Self::Pmc(p) => write!(f, "{p}"),
            Self::OpenAlex(w) => write!(f, "{w}"),
        }
    }
}

/// A name that owns its text: the registry's and the tables' form. Orders
/// by namespace in [`Key::ALL`] order, then by value, so a sorted list of
/// names is deterministic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OwnedName {
    #[allow(missing_docs)]
    Doi(Box<str>),
    #[allow(missing_docs)]
    Pmid(Pmid),
    #[allow(missing_docs)]
    Pmc(PmcId),
    #[allow(missing_docs)]
    OpenAlex(OpenAlexId),
}

impl OwnedName {
    /// The name, borrowed.
    #[must_use]
    pub fn as_name(&self) -> Name<'_> {
        match self {
            Self::Doi(d) => Name::Doi(d),
            Self::Pmid(p) => Name::Pmid(*p),
            Self::Pmc(p) => Name::Pmc(*p),
            Self::OpenAlex(w) => Name::OpenAlex(*w),
        }
    }

    /// The namespace.
    #[must_use]
    pub fn key(&self) -> Key {
        self.as_name().key()
    }

    /// Parses the text form [`Name`] displays; see [`Name::parse`].
    ///
    /// # Errors
    ///
    /// As [`Name::parse`].
    pub fn parse(text: &str) -> Result<Self, ParseNameError> {
        Name::parse(text).map(|n| n.to_owned())
    }
}

impl fmt::Display for OwnedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.as_name(), f)
    }
}

impl From<Name<'_>> for OwnedName {
    fn from(name: Name<'_>) -> Self {
        name.to_owned()
    }
}

/// As its text form, in every format: the merged sets are read back with
/// the text borrowed.
impl Serialize for Name<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Name<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <&'de str>::deserialize(deserializer)?;
        Self::parse(text).map_err(D::Error::custom)
    }
}

/// As its text form, in every format: the registry is JSON lines and
/// meant for eyes.
impl Serialize for OwnedName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for OwnedName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Self::parse(&text).map_err(D::Error::custom)
    }
}

/// A text that is not a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseNameError {
    /// As given.
    pub text: String,
    /// Why.
    pub reason: String,
}

impl fmt::Display for ParseNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a name: {}", self.text, self.reason)
    }
}

impl std::error::Error for ParseNameError {}

/// One record saying a name is its own (merge plan §3.1): the name, the
/// field it came from, and whether it is the record's native id or a
/// foreign id it carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assertion<'a> {
    #[allow(missing_docs)]
    pub name: Name<'a>,
    /// The normalized field, for the report and the verdict.
    pub field: &'static str,
    /// The source's own identifier for the record, which it holds by
    /// right; a claim otherwise.
    pub native: bool,
}

#[cfg(test)]
mod tests {
    use model::{OpenAlexId, PmcId, Pmid};

    use super::{Name, OwnedName};

    #[test]
    fn names_display_and_parse_canonically() {
        let names = [
            OwnedName::Doi("10.1000/x".into()),
            OwnedName::Pmid(Pmid::from_digits("123").unwrap()),
            OwnedName::Pmc(PmcId::from_prefixed("PMC45").unwrap()),
            OwnedName::OpenAlex(OpenAlexId::from_bare("W6").unwrap()),
        ];
        let texts = ["doi:10.1000/x", "pmid:123", "pmc:PMC45", "openalex:W6"];
        for (name, text) in names.iter().zip(texts) {
            assert_eq!(name.to_string(), text);
            assert_eq!(OwnedName::parse(text).unwrap(), *name);
            assert_eq!(serde_json::to_string(name).unwrap(), format!("{text:?}"));
            assert_eq!(
                serde_json::from_str::<OwnedName>(&format!("{text:?}")).unwrap(),
                *name
            );
            assert_eq!(name.as_name().to_owned(), *name);
        }
        for name in &names {
            let bytes = postcard::to_allocvec(&name.as_name()).unwrap();
            let back: Name<'_> = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(back.to_owned(), *name, "borrowed through postcard");
        }
        assert!(OwnedName::parse("10.1000/x").is_err(), "no namespace");
        assert!(OwnedName::parse("isbn:1").is_err(), "unknown namespace");
        assert!(OwnedName::parse("doi:10.1000/X").is_err(), "not canonical");
        assert!(OwnedName::parse("pmid:0123").is_err());
        assert!(
            OwnedName::parse("pmc:45").is_err(),
            "the prefix is the form"
        );
        assert!(OwnedName::parse("openalex:6").is_err());
        let mut sorted = names.to_vec();
        sorted.reverse();
        sorted.sort();
        assert_eq!(sorted, names, "namespace order, then value");
        assert_eq!(
            Name::Pmid(Pmid::from_digits("7").unwrap()).number(),
            Some(7)
        );
        assert_eq!(Name::Doi("10.1/a").number(), None);
    }
}
