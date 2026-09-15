//! Languages, as an ISO 639-2 vocabulary.
//!
//! Sources write language three ways: PubMed uses ISO 639-2/B bibliographic
//! codes (`fre`, `ger`, `chi`), OpenAlex uses ISO 639-1 alpha-2 (`fr`), and
//! others use the terminologic alpha-3 (`fra`). [`Lang`] is the language
//! itself, keyed by its terminologic code, with one strict entry point per
//! form — a source that switches forms fails at that source (plan §6).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

mod table;

use table::{ALPHA_2, BIBLIOGRAPHIC, ENTRIES};

/// A language in ISO 639-2. Two bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lang(u16);

/// A code that is not in the vocabulary, in the form requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Error;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not an ISO 639-2 code in the expected form")
    }
}

impl std::error::Error for Error {}

fn index_of<T: Copy>(sorted: &[(&str, T)], code: &str) -> Option<T> {
    sorted
        .binary_search_by(|(k, _)| (*k).cmp(code))
        .ok()
        .map(|i| sorted[i].1)
}

impl Lang {
    /// The bibliographic form, as PubMed writes it: `fre` for French, and
    /// the plain alpha-3 for languages with no separate bibliographic code.
    /// The terminologic `fra` is rejected here.
    ///
    /// # Errors
    ///
    /// [`Error`] for any code not in the vocabulary in that form.
    pub fn from_bibliographic(code: &str) -> Result<Self, Error> {
        if let Some(i) = index_of(&BIBLIOGRAPHIC, code) {
            return Ok(Self(i));
        }
        let lang = Self::from_terminologic(code)?;
        if ENTRIES[usize::from(lang.0)].bibliographic.is_some() {
            return Err(Error);
        }
        Ok(lang)
    }

    /// The terminologic alpha-3 form: `fra`, `deu`, `zho`, `eng`.
    ///
    /// # Errors
    ///
    /// [`Error`] for any code not in the vocabulary in that form.
    pub fn from_terminologic(code: &str) -> Result<Self, Error> {
        ENTRIES
            .binary_search_by(|e| e.alpha_3.cmp(code))
            .map(|i| Self(u16::try_from(i).unwrap_or(u16::MAX)))
            .map_err(|_| Error)
    }

    /// The ISO 639-1 alpha-2 form: `fr`, `de`, `zh`, `en`.
    ///
    /// # Errors
    ///
    /// [`Error`] for a code with no alpha-2 form, or not in the vocabulary.
    pub fn from_alpha_2(code: &str) -> Result<Self, Error> {
        index_of(&ALPHA_2, code).map(Self).ok_or(Error)
    }

    fn entry(self) -> &'static table::Entry {
        &ENTRIES[usize::from(self.0)]
    }

    /// The terminologic alpha-3 code.
    #[must_use]
    pub fn alpha_3(self) -> &'static str {
        self.entry().alpha_3
    }

    /// The bibliographic alpha-3 code, which is the terminologic one for
    /// most languages.
    #[must_use]
    pub fn bibliographic(self) -> &'static str {
        self.entry().bibliographic.unwrap_or(self.entry().alpha_3)
    }

    /// The alpha-2 code, for the languages that have one.
    #[must_use]
    pub fn alpha_2(self) -> Option<&'static str> {
        self.entry().alpha_2
    }

    /// The English name, as ISO lists it (`Spanish; Castilian`).
    #[must_use]
    pub fn name(self) -> &'static str {
        self.entry().name
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.alpha_3())
    }
}

impl Serialize for Lang {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::human::serialize(self.alpha_3(), self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for Lang {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::human::deserialize(
            deserializer,
            "language",
            |i: u16| (usize::from(i) < ENTRIES.len()).then_some(Self(i)),
            |s| Self::from_terminologic(s).ok(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Lang, table::ENTRIES};

    #[test]
    fn table_is_sorted_and_well_formed() {
        assert!(ENTRIES.windows(2).all(|w| w[0].alpha_3 < w[1].alpha_3));
        assert!(ENTRIES.iter().all(|e| e.alpha_3.len() == 3));
        assert!(ENTRIES.len() > 480);
    }

    #[test]
    fn pubmed_bibliographic_forms() {
        for (code, canonical, name) in [
            ("eng", "eng", "English"),
            ("fre", "fra", "French"),
            ("ger", "deu", "German"),
            ("chi", "zho", "Chinese"),
            ("jpn", "jpn", "Japanese"),
            ("tur", "tur", "Turkish"),
            ("dan", "dan", "Danish"),
            ("swe", "swe", "Swedish"),
            ("spa", "spa", "Spanish; Castilian"),
            ("und", "und", "Undetermined"),
        ] {
            let lang = Lang::from_bibliographic(code).unwrap();
            assert_eq!(lang.alpha_3(), canonical, "{code}");
            assert_eq!(lang.bibliographic(), code);
            assert_eq!(lang.name(), name);
        }
    }

    #[test]
    fn forms_are_not_interchangeable() {
        assert_eq!(Lang::from_bibliographic("fra"), Err(Error));
        assert_eq!(Lang::from_terminologic("fre"), Err(Error));
        assert_eq!(Lang::from_alpha_2("fre"), Err(Error));
        assert_eq!(Lang::from_alpha_2("fr"), Lang::from_terminologic("fra"));
        assert_eq!(Lang::from_alpha_2("en"), Lang::from_bibliographic("eng"));
        for bad in ["", "en", "ENG", "Eng", "eng ", "english", "xxx"] {
            assert_eq!(Lang::from_bibliographic(bad), Err(Error), "{bad:?}");
        }
    }

    #[test]
    fn size() {
        assert_eq!(size_of::<Lang>(), 2);
    }
}
