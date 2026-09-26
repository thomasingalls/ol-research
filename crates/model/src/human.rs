//! Two serialized forms for the packed types: their canonical text for a
//! human-readable format (JSON, in `dump`) and the packed number for a
//! binary one (postcard, in the normalized layer). serde tells which is
//! asked for; the derive-based types with an obvious JSON shape — dates as
//! structs, claims as enums — do not need this.

use std::borrow::Cow;
use std::fmt::Display;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Serializes `text` for a human-readable format, `packed` otherwise.
pub(crate) fn serialize<S: Serializer, T: Display + ?Sized, P: Serialize>(
    text: &T,
    packed: P,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if serializer.is_human_readable() {
        serializer.collect_str(text)
    } else {
        packed.serialize(serializer)
    }
}

/// Deserializes through `from_text` for a human-readable format, through
/// `from_packed` otherwise; either returning `None` is an error naming
/// `what`.
pub(crate) fn deserialize<'de, D, P, T>(
    deserializer: D,
    what: &'static str,
    from_packed: impl FnOnce(P) -> Option<T>,
    from_text: impl FnOnce(&str) -> Option<T>,
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    P: Deserialize<'de>,
{
    if deserializer.is_human_readable() {
        let text = Cow::<'de, str>::deserialize(deserializer)?;
        from_text(&text).ok_or_else(|| D::Error::custom(format!("not a {what}: {text:?}")))
    } else {
        let packed = P::deserialize(deserializer)?;
        from_packed(packed).ok_or_else(|| D::Error::custom(format!("not a packed {what}")))
    }
}

#[cfg(test)]
mod tests {
    use crate::id::{InstitutionId, OpenAlexId};
    use crate::{
        CountryCode, CurrencyCode, Date, DateTime, Isbn, Issn, Lang, Orcid, PartialDate, Qid, Ror,
    };

    fn roundtrip<T>(value: &T, json: &str, packed_len: usize)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        assert_eq!(serde_json::to_string(value).unwrap(), json);
        assert_eq!(&serde_json::from_str::<T>(json).unwrap(), value);
        let bytes = postcard::to_stdvec(value).unwrap();
        assert_eq!(bytes.len(), packed_len, "{json}: {bytes:?}");
        assert_eq!(&postcard::from_bytes::<T>(&bytes).unwrap(), value);
    }

    #[test]
    fn text_for_eyes_numbers_for_disk() {
        roundtrip(
            &Orcid::from_hyphenated("0000-0002-4644-2459").unwrap(),
            "\"0000-0002-4644-2459\"",
            4,
        );
        roundtrip(&Ror::from_bare("052gg0110").unwrap(), "\"052gg0110\"", 4);
        roundtrip(&Qid::from_prefixed("Q182973").unwrap(), "\"Q182973\"", 3);
        roundtrip(&Issn::parse("0004-637X").unwrap(), "\"0004-637X\"", 2);
        roundtrip(&Lang::from_alpha_2("fr").unwrap(), "\"fra\"", 2);
        roundtrip(&CountryCode::parse("US").unwrap(), "\"US\"", 2);
        roundtrip(&CurrencyCode::parse("USD").unwrap(), "\"USD\"", 3);
        roundtrip(
            &OpenAlexId::from_bare("W3029093293").unwrap(),
            "\"W3029093293\"",
            5,
        );
        roundtrip(
            &InstitutionId::from_bare("I40120149").unwrap(),
            "\"I40120149\"",
            4,
        );
        roundtrip(&Date::parse_iso("2001-01-01").unwrap(), "\"2001-01-01\"", 4);
        roundtrip(
            &DateTime::parse_utc_millis("2025-07-25T07:35:27.710Z").unwrap(),
            "\"2025-07-25T07:35:27.710Z\"",
            8,
        );
        roundtrip(
            &DateTime::parse_utc("2014-07-05T15:19:03Z").unwrap(),
            "\"2014-07-05T15:19:03Z\"",
            8,
        );
        // Derived shapes stay structural in both formats.
        roundtrip(
            &PartialDate::from_numbers(2020, Some(3), None).unwrap(),
            "{\"YearMonth\":[2020,\"March\"]}",
            4,
        );
        roundtrip(
            &Isbn::parse("9780306406157").unwrap(),
            "\"9780306406157\"",
            7,
        );
        roundtrip(&Isbn::parse("1558602380").unwrap(), "\"1558602380\"", 5);
        assert!(
            serde_json::from_str::<Orcid>("\"0000-0002-4644-2450\"").is_err(),
            "checksum"
        );
        assert!(
            serde_json::from_str::<Lang>("\"fr\"").is_err(),
            "alpha-2 is not the text form"
        );
        assert!(postcard::from_bytes::<Ror>(&postcard::to_stdvec(&(1u32 << 31)).unwrap()).is_err());
        assert!(postcard::from_bytes::<Qid>(&postcard::to_stdvec(&0u32).unwrap()).is_err());
        assert!(
            serde_json::from_str::<Qid>("\"182973\"").is_err(),
            "the letter is the form"
        );
    }
}
