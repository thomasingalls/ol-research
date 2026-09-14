//! Crossref's date objects.
//!
//! A `date-parts` value is `[[year, month, day]]`, `[[year, month]]`,
//! `[[year]]` or `[[null]]` — an array of exactly one array, integers only,
//! or the single null that means "no date" (34 of 1,000 works, all in
//! `issued`). [`DateParts`] admits exactly those shapes. The integers are
//! kept as the source wrote them; whether `2027` or month `13` is a date is
//! the normalizer's question.

use std::borrow::Cow;
use std::fmt;

use serde::Deserialize;
use serde::de::{self, Deserializer, SeqAccess, Visitor};

/// The parts of a `date-parts` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DateParts {
    /// The year, absent only for `[[null]]`.
    pub year: Option<u32>,
    /// The month, when given.
    pub month: Option<u32>,
    /// The day, when given; only with a month.
    pub day: Option<u32>,
}

impl DateParts {
    /// True for `[[null]]`.
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.year.is_none()
    }
}

/// One inner array: `[y]`, `[y, m]`, `[y, m, d]` or `[null]`.
struct Inner(DateParts);

impl<'de> Deserialize<'de> for Inner {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Inner;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("[year, month?, day?] or [null]")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Inner, A::Error> {
                let mut parts = [None, None, None];
                let mut len = 0;
                while let Some(part) = seq.next_element::<Option<u32>>()? {
                    if len == 3 {
                        return Err(de::Error::custom("date-parts with more than three parts"));
                    }
                    parts[len] = part;
                    len += 1;
                }
                match (len, parts) {
                    (0, _) => Err(de::Error::custom("empty date-parts")),
                    (1, [None, ..]) => Ok(Inner(DateParts::default())),
                    _ if parts[..len].iter().any(Option::is_none) => Err(de::Error::custom(
                        "null inside date-parts other than [null]",
                    )),
                    _ => Ok(Inner(DateParts {
                        year: parts[0],
                        month: parts[1],
                        day: parts[2],
                    })),
                }
            }
        }
        deserializer.deserialize_seq(V)
    }
}

impl<'de> Deserialize<'de> for DateParts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = DateParts;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an array of exactly one date-parts array")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<DateParts, A::Error> {
                let Some(Inner(parts)) = seq.next_element::<Inner>()? else {
                    return Err(de::Error::custom("date-parts with no entries"));
                };
                if seq.next_element::<de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom("date-parts with more than one entry"));
                }
                Ok(parts)
            }
        }
        deserializer.deserialize_seq(V)
    }
}

/// A partial date: `issued`, `published-*`, `posted`, `accepted`, …
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct PartialDate {
    /// The parts.
    pub date_parts: DateParts,
}

/// A full timestamp: `created`, `deposited`, license `start`, update `updated`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Stamp<'a> {
    /// Always three parts here.
    pub date_parts: DateParts,
    /// ISO 8601, `2014-07-05T15:19:03Z`.
    #[serde(borrow)]
    pub date_time: Cow<'a, str>,
    /// Milliseconds since the epoch; negative before 1970.
    pub timestamp: i64,
}

/// `indexed`: a [`Stamp`] plus the indexer version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct IndexedStamp<'a> {
    /// Always three parts here.
    pub date_parts: DateParts,
    /// ISO 8601.
    #[serde(borrow)]
    pub date_time: Cow<'a, str>,
    /// Milliseconds since the epoch.
    pub timestamp: i64,
    /// The indexer version, e.g. `3.50.1`; present on most works.
    #[serde(borrow)]
    pub version: Option<Cow<'a, str>>,
}

#[cfg(test)]
mod tests {
    use super::{DateParts, PartialDate};

    fn parts(s: &str) -> Result<DateParts, serde_json::Error> {
        serde_json::from_str(s)
    }

    #[test]
    fn the_four_observed_shapes() {
        assert_eq!(
            parts("[[2020,3,15]]").unwrap(),
            DateParts {
                year: Some(2020),
                month: Some(3),
                day: Some(15)
            }
        );
        assert_eq!(
            parts("[[2020,3]]").unwrap(),
            DateParts {
                year: Some(2020),
                month: Some(3),
                day: None
            }
        );
        assert_eq!(
            parts("[[2020]]").unwrap(),
            DateParts {
                year: Some(2020),
                month: None,
                day: None
            }
        );
        assert!(parts("[[null]]").unwrap().is_null());
    }

    #[test]
    fn nothing_else() {
        for bad in [
            "[]",
            "[[]]",
            "[[2020],[2021]]",
            "[[2020,null,5]]",
            "[[null,3]]",
            "[[2020,3,15,1]]",
            "[2020]",
            "[[\"2020\"]]",
            "[[-1]]",
        ] {
            assert!(parts(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn partial_date_rejects_extra_keys() {
        assert!(serde_json::from_str::<PartialDate>(r#"{"date-parts":[[2020]]}"#).is_ok());
        assert!(
            serde_json::from_str::<PartialDate>(r#"{"date-parts":[[2020]],"timestamp":1}"#)
                .is_err()
        );
    }
}
