use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::num::NonZeroU64;
use std::str::FromStr;

use super::{ParseError, parse_digits, strip_literal};

/// An OpenAlex entity identifier: one uppercase letter naming the entity
/// type, then a positive decimal. `KIND` is that letter; the aliases below
/// name the types the snapshot uses.
///
/// Observed forms (plan §4a; `updated_date=2025-07-25/part_0000.gz` of the
/// 2026-06-26 snapshot):
///
/// * `https://openalex.org/W3029093293` — every identifier the snapshot
///   emits, for every entity type, is this URL form — [`EntityId::from_url`].
/// * Bare `W3029093293`, the API's short form — [`FromStr`].
///
/// The largest work id in that file is `W4410348178`, above `u32::MAX`,
/// and the other entity types have ids above it too (`I4210116856`,
/// `S4306402636`, `F4320321001`), so this is a `u64`. `Option` is eight
/// bytes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct EntityId<const KIND: char>(NonZeroU64);

/// A work, `W…`: the native identifier this pipeline is committed to.
pub type OpenAlexId = EntityId<'W'>;
/// An author, `A…`.
pub type AuthorId = EntityId<'A'>;
/// A source — journal, repository, conference, … — `S…`.
pub type SourceId = EntityId<'S'>;
/// An institution, `I…`.
pub type InstitutionId = EntityId<'I'>;
/// A publisher, `P…`.
pub type PublisherId = EntityId<'P'>;
/// A funder, `F…`.
pub type FunderId = EntityId<'F'>;
/// An award (a grant), `G…`.
pub type AwardId = EntityId<'G'>;
/// A topic, `T…`.
pub type TopicId = EntityId<'T'>;
/// A concept, `C…` (deprecated by OpenAlex in favour of topics).
pub type ConceptId = EntityId<'C'>;

impl<const KIND: char> EntityId<KIND> {
    /// The entity letter.
    pub const ENTITY_PREFIX: char = KIND;
    /// The prefix OpenAlex puts in front of every identifier it emits.
    pub const URL_PREFIX: &'static str = "https://openalex.org/";

    /// Wraps a raw value; `None` for zero.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// The numeric value, without the letter.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    /// Parses the bare `W…` form.
    ///
    /// # Errors
    ///
    /// [`ParseError::BadPrefix`] unless the input starts with the entity
    /// letter; then anything other than a positive decimal with no sign, no
    /// leading zero and no surrounding whitespace, or a value above
    /// `u64::MAX`. Offsets are relative to the whole input.
    pub fn from_bare(s: &str) -> Result<Self, ParseError> {
        const {
            assert!(
                KIND.is_ascii_uppercase(),
                "an entity kind is one uppercase ASCII letter"
            );
        }
        let digits = s.strip_prefix(KIND).ok_or(ParseError::BadPrefix)?;
        parse_digits(digits, u64::MAX)
            .map_err(|e| e.shift(1))
            .and_then(|v| NonZeroU64::new(v).map(Self).ok_or(ParseError::Zero))
    }

    /// Parses the URL form the snapshot and API emit.
    ///
    /// # Errors
    ///
    /// [`ParseError::BadPrefix`] unless the input starts with exactly
    /// [`Self::URL_PREFIX`]; then as [`Self::from_bare`], with offsets
    /// relative to the whole input.
    pub fn from_url(s: &str) -> Result<Self, ParseError> {
        let rest = strip_literal(s, Self::URL_PREFIX)?;
        Self::from_bare(rest).map_err(|e| e.shift(Self::URL_PREFIX.len()))
    }
}

impl<const KIND: char> FromStr for EntityId<KIND> {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_bare(s)
    }
}

impl<const KIND: char> fmt::Display for EntityId<KIND> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{KIND}{}", self.0)
    }
}

impl<const KIND: char> fmt::Debug for EntityId<KIND> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{KIND}{}", self.0)
    }
}

impl<const KIND: char> From<EntityId<KIND>> for u64 {
    fn from(id: EntityId<KIND>) -> Self {
        id.get()
    }
}

/// The organization hosting a source: an institution (`I…`) for a
/// repository, a publisher (`P…`) for a journal. One field, two entity
/// types, as the snapshot writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostOrganization {
    #[allow(missing_docs)]
    Institution(InstitutionId),
    #[allow(missing_docs)]
    Publisher(PublisherId),
}

impl HostOrganization {
    /// Parses the URL form, deciding the entity type by its letter.
    ///
    /// # Errors
    ///
    /// [`ParseError::BadPrefix`] for any letter other than `I` or `P`, or
    /// the wrong URL prefix; then as [`EntityId::from_bare`], with offsets
    /// relative to the whole input.
    pub fn from_url(s: &str) -> Result<Self, ParseError> {
        let rest = strip_literal(s, OpenAlexId::URL_PREFIX)?;
        match rest.as_bytes().first() {
            Some(b'I') => InstitutionId::from_bare(rest).map(Self::Institution),
            Some(b'P') => PublisherId::from_bare(rest).map(Self::Publisher),
            _ => Err(ParseError::BadPrefix),
        }
        .map_err(|e| e.shift(OpenAlexId::URL_PREFIX.len()))
    }
}

impl fmt::Display for HostOrganization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Institution(id) => fmt::Display::fmt(id, f),
            Self::Publisher(id) => fmt::Display::fmt(id, f),
        }
    }
}

impl<const KIND: char> Serialize for EntityId<KIND> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::human::serialize(self, self.0.get(), serializer)
    }
}

impl<'de, const KIND: char> Deserialize<'de> for EntityId<KIND> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::human::deserialize(
            deserializer,
            "OpenAlex id",
            |n: u64| Self::new(n),
            |s| Self::from_bare(s).ok(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorId, FunderId, HostOrganization, InstitutionId, OpenAlexId, ParseError, PublisherId,
        SourceId,
    };

    /// First three work `id`s in the OpenAlex part file.
    const OBSERVED: [&str; 3] = [
        "https://openalex.org/W3029093293",
        "https://openalex.org/W3025147924",
        "https://openalex.org/W3025437713",
    ];
    /// The largest work `id` in the part file: above `u32::MAX`.
    const OBSERVED_MAX: &str = "https://openalex.org/W4410348178";

    #[test]
    fn parses_observed_url_form() {
        for s in OBSERVED {
            let id = OpenAlexId::from_url(s).unwrap();
            assert_eq!(format!("{}{id}", OpenAlexId::URL_PREFIX), s);
        }
    }

    #[test]
    fn observed_maximum_needs_u64() {
        let id = OpenAlexId::from_url(OBSERVED_MAX).unwrap();
        assert_eq!(id.get(), 4_410_348_178);
        assert!(id.get() > u64::from(u32::MAX));
    }

    #[test]
    fn parses_bare_form() {
        let id: OpenAlexId = "W3029093293".parse().unwrap();
        assert_eq!(id.to_string(), "W3029093293");
        assert_eq!(format!("{id:?}"), "W3029093293");
        assert_eq!(id, OpenAlexId::from_url(OBSERVED[0]).unwrap());
    }

    #[test]
    fn forms_are_not_interchangeable() {
        assert_eq!(
            OpenAlexId::from_bare(OBSERVED[0]),
            Err(ParseError::BadPrefix)
        );
        assert_eq!(OpenAlexId::from_url("W1"), Err(ParseError::BadPrefix));
    }

    #[test]
    fn entity_prefix_is_exact() {
        for s in ["", "w1", "A123", "S1", "1", " W1"] {
            assert_eq!(
                OpenAlexId::from_bare(s),
                Err(ParseError::BadPrefix),
                "{s:?}"
            );
        }
        assert_eq!(
            OpenAlexId::from_url("https://openalex.org/w1"),
            Err(ParseError::BadPrefix)
        );
        assert_eq!(
            OpenAlexId::from_url("http://openalex.org/W1"),
            Err(ParseError::BadPrefix)
        );
    }

    #[test]
    fn other_entities_have_their_own_letters() {
        // Observed in the part file: an author, a source, an institution and
        // a funder, every one above `u32::MAX`.
        let a = AuthorId::from_url("https://openalex.org/A5020806913").unwrap();
        assert_eq!(a.to_string(), "A5020806913");
        let s = SourceId::from_url("https://openalex.org/S4306402636").unwrap();
        assert_eq!(s.get(), 4_306_402_636);
        let i = InstitutionId::from_url("https://openalex.org/I4210116856").unwrap();
        assert_eq!(format!("{i:?}"), "I4210116856");
        let f = FunderId::from_url("https://openalex.org/F4320321001").unwrap();
        assert!(f.get() > u64::from(u32::MAX));
        // The letter is part of the type: a work id is not an author id.
        assert_eq!(AuthorId::from_url(OBSERVED[0]), Err(ParseError::BadPrefix));
        assert_eq!(
            InstitutionId::from_bare("A5020806913"),
            Err(ParseError::BadPrefix)
        );
        assert_eq!(AuthorId::ENTITY_PREFIX, 'A');
    }

    #[test]
    fn host_organization_is_an_institution_or_a_publisher() {
        assert_eq!(
            HostOrganization::from_url("https://openalex.org/I40120149"),
            Ok(HostOrganization::Institution(
                InstitutionId::from_bare("I40120149").unwrap()
            ))
        );
        assert_eq!(
            HostOrganization::from_url("https://openalex.org/P4310320990"),
            Ok(HostOrganization::Publisher(
                PublisherId::from_bare("P4310320990").unwrap()
            ))
        );
        assert_eq!(
            HostOrganization::from_url("https://openalex.org/P4310320990").map(|h| h.to_string()),
            Ok("P4310320990".to_owned())
        );
        assert_eq!(
            HostOrganization::from_url("https://openalex.org/S1"),
            Err(ParseError::BadPrefix)
        );
        assert_eq!(
            HostOrganization::from_url("https://openalex.org/"),
            Err(ParseError::BadPrefix)
        );
        assert_eq!(
            HostOrganization::from_url("https://openalex.org/I01"),
            Err(ParseError::LeadingZero)
        );
        assert_eq!(
            HostOrganization::from_url("https://openalex.org/P1 "),
            Err(ParseError::NotDigit {
                offset: OpenAlexId::URL_PREFIX.len() + 2
            })
        );
    }

    #[test]
    fn offsets_cover_the_whole_input() {
        assert_eq!(OpenAlexId::from_bare("W"), Err(ParseError::Empty));
        assert_eq!(OpenAlexId::from_bare("W0"), Err(ParseError::Zero));
        assert_eq!(OpenAlexId::from_bare("W01"), Err(ParseError::LeadingZero));
        assert_eq!(
            OpenAlexId::from_bare("W1 "),
            Err(ParseError::NotDigit { offset: 2 })
        );
        let p = OpenAlexId::URL_PREFIX;
        assert_eq!(
            OpenAlexId::from_url(&format!("{p}W1/")),
            Err(ParseError::NotDigit {
                offset: p.len() + 2
            })
        );
    }

    #[test]
    fn width_is_u64() {
        assert_eq!(
            OpenAlexId::from_bare("W18446744073709551615").map(OpenAlexId::get),
            Ok(u64::MAX)
        );
        assert_eq!(
            OpenAlexId::from_bare("W18446744073709551616"),
            Err(ParseError::Overflow)
        );
    }

    #[test]
    fn option_is_niche_packed() {
        assert_eq!(size_of::<Option<OpenAlexId>>(), 8);
        assert_eq!(size_of::<OpenAlexId>(), 8);
        assert_eq!(size_of::<Option<HostOrganization>>(), 16);
    }

    #[test]
    fn new_and_get_roundtrip() {
        assert_eq!(OpenAlexId::new(0), None);
        assert_eq!(
            OpenAlexId::new(4_410_348_178).map(u64::from),
            Some(4_410_348_178)
        );
    }
}
