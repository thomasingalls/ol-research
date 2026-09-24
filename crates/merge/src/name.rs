//! Names: what the world calls a work (merge plan §2), and a record's
//! assertions of them (§3.1).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use ingest::Key;
use model::{Doi, OpenAlexId, PmcId, Pmid, Registrant};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::publisher;

/// A name borrowed from a record: a namespace and a value. A DOI is
/// canonical text, lowercase and bare, as `model::Doi` holds it; the rest
/// are the model's packed ids.
#[derive(Debug, Clone, Copy)]
pub enum Name<'a> {
    #[allow(missing_docs)]
    Doi(&'a str),
    #[allow(missing_docs)]
    Pmid(Pmid),
    #[allow(missing_docs)]
    Pmc(PmcId),
    #[allow(missing_docs)]
    OpenAlex(OpenAlexId),
    /// A publisher's own identifier for the article, scoped by the DOI
    /// registrant that issued it (merge plan §3.1). The text is the value
    /// exactly as it was written — by the record, or by the line of the
    /// registry this was read back from — and it stays that way through
    /// [`Name::to_owned`]. The *canonical spelling* is a view of it,
    /// computed on demand: the same text unless it is Elsevier's
    /// punctuated printed PII, whose canonical spelling is bare
    /// ([`crate::publisher`]).
    ///
    /// Equality, hash, order and display all run over that view, so the
    /// two spellings of one article's PII are one name without either
    /// layer being rewritten to make them so — and so that
    /// [`publisher::ELSEVIER`] can grow. It could not while the stored
    /// form was the canonical one: adding a registrant unspelled every
    /// name already sealed under it and the next merge died reading the
    /// registry (2026-09-19). A value's spelling is now a fact about the
    /// record that wrote it, never a fact the merged layer depends on.
    ///
    /// [`publisher::ELSEVIER`]: crate::publisher::ELSEVIER
    Publisher(Registrant, &'a str),
}

/// Over the canonical spelling, never the written one: two records that
/// spelled one publisher id differently assert one name.
impl PartialEq for Name<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Doi(a), Self::Doi(b)) => a == b,
            (Self::Pmid(a), Self::Pmid(b)) => a == b,
            (Self::Pmc(a), Self::Pmc(b)) => a == b,
            (Self::OpenAlex(a), Self::OpenAlex(b)) => a == b,
            (Self::Publisher(ra, a), Self::Publisher(rb, b)) => {
                ra == rb
                    && publisher::canonical_bytes(*ra, a).eq(publisher::canonical_bytes(*rb, b))
            }
            _ => false,
        }
    }
}

impl Eq for Name<'_> {}

/// As [`PartialEq`]: the canonical spelling's bytes, terminated the way
/// `str`'s own hash terminates them so that no two values run together.
impl Hash for Name<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Doi(d) => d.hash(state),
            Self::Pmid(p) => p.hash(state),
            Self::Pmc(p) => p.hash(state),
            Self::OpenAlex(w) => w.hash(state),
            Self::Publisher(registrant, value) => {
                registrant.hash(state);
                for byte in publisher::canonical_bytes(*registrant, value) {
                    state.write_u8(byte);
                }
                state.write_u8(0xff);
            }
        }
    }
}

/// As [`PartialEq`], so that a name that sorts equal to another *is*
/// that other: the namespace in [`Key::ALL`] order — which is [`Key`]'s
/// own — then the value, a publisher id's by its canonical spelling.
///
/// Two spellings of one PII compare `Equal` here and render the same
/// text, so a sorted list of names is deterministic whichever of them a
/// run happened to read first.
impl Ord for Name<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Doi(a), Self::Doi(b)) => a.cmp(b),
            (Self::Pmid(a), Self::Pmid(b)) => a.cmp(b),
            (Self::Pmc(a), Self::Pmc(b)) => a.cmp(b),
            (Self::OpenAlex(a), Self::OpenAlex(b)) => a.cmp(b),
            (Self::Publisher(ra, a), Self::Publisher(rb, b)) => ra.cmp(rb).then_with(|| {
                publisher::canonical_bytes(*ra, a).cmp(publisher::canonical_bytes(*rb, b))
            }),
            _ => self.key().cmp(&other.key()),
        }
    }
}

impl PartialOrd for Name<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
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
            Self::Publisher(..) => Key::Publisher,
        }
    }

    /// The number of a numeric name; `None` for a DOI or a publisher id.
    #[must_use]
    pub const fn number(&self) -> Option<u64> {
        match self {
            Self::Doi(_) | Self::Publisher(..) => None,
            Self::Pmid(p) => Some(p.get() as u64),
            Self::Pmc(p) => Some(p.get() as u64),
            Self::OpenAlex(w) => Some(w.get()),
        }
    }

    /// Parses the text form this type displays, the text of a DOI or a
    /// publisher id borrowed from `text`, through the model's own parsers.
    ///
    /// A publisher id's namespace is `reg1016` — the registrant, spelled
    /// so that it cannot be mistaken for one. A registrant-shaped
    /// namespace (`10.1016:`, or its bare digits) is refused here rather
    /// than merely unknown, because the one thing the merged layer must
    /// not grow is a second identifier shaped like a DOI (decision log,
    /// 2026-09-19).
    ///
    /// A publisher id is read in whatever spelling it was written in.
    /// The other namespaces still refuse a value their own canonical
    /// text differs from — an uppercase DOI is a different string for a
    /// rule that has not moved since the model was written — but a
    /// publisher id's canonical spelling depends on
    /// [`publisher::ELSEVIER`], which is ours and changes. Refusing a
    /// value that list would rewrite made every name already sealed
    /// under a new registrant unreadable, and that is how the merge of
    /// 2026-09-19 died. The `publisher::check` rules — shape, length,
    /// printable ASCII, article numbers, a DOI — are facts about the
    /// value and still hold.
    ///
    /// [`publisher::ELSEVIER`]: crate::publisher::ELSEVIER
    ///
    /// # Errors
    ///
    /// No namespace, an unknown one, a registrant-shaped one, or a value
    /// its parser refuses.
    pub fn parse(text: &'a str) -> Result<Self, ParseNameError> {
        let err = |reason: String| ParseNameError {
            text: text.to_owned(),
            reason,
        };
        let (key, value) = text
            .split_once(':')
            .ok_or_else(|| err("no namespace".to_owned()))?;
        if Registrant::parse(key).is_ok()
            || (!key.is_empty() && key.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(err(
                "a publisher id's namespace is reg1016, never a registrant".to_owned(),
            ));
        }
        if let Some(digits) = key.strip_prefix("reg") {
            let registrant = publisher::registrant_of(digits)
                .ok_or_else(|| err(format!("{key:?} names no registrant")))?;
            publisher::check(value).map_err(|why| err(why.to_string()))?;
            return Ok(Self::Publisher(registrant, value));
        }
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
            // The key names the family, not a namespace: a publisher id
            // without the registrant that issued it is scoped by nothing.
            Key::Publisher => {
                return Err(err(
                    "a publisher id's namespace is its registrant, `reg1016`".to_owned(),
                ));
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
            // The spelling is kept, not settled: owning the text says
            // nothing about which of two spellings is the canonical one,
            // and the answer would go stale the day the list changes.
            Self::Publisher(registrant, value) => OwnedName::Publisher(registrant, value.into()),
        }
    }
}

/// `doi:10.1000/x`, `pmid:123`, `pmc:PMC123`, `openalex:W123`: the
/// namespace's [`Key`] name, a colon, the model's canonical text. A
/// publisher id names its registrant instead — `reg1016:S0006349525001234`
/// — and writes the canonical spelling of its value.
///
/// This is where a publisher id's spelling is promoted, and the only
/// place: a name is stored as it was written and rendered canonical, so
/// every string form — the registry's lines, a merged row, `merge dump`,
/// the names transducer's keys, the serving layer's rows, and every map
/// in the workspace keyed by a name's text — gets the one spelling
/// without knowing the rule. Rendering rather than storing it is also
/// what keeps those files deterministic: two records may write one PII
/// two ways and either may be read first, but what is written out does
/// not depend on which.
impl fmt::Display for Name<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Publisher(registrant, value) => write!(
                f,
                "reg{}:{}",
                registrant.get(),
                publisher::canonical(*registrant, value)
            ),
            Self::Doi(d) => write!(f, "{}:{d}", self.key()),
            Self::Pmid(p) => write!(f, "{}:{p}", self.key()),
            Self::Pmc(p) => write!(f, "{}:{p}", self.key()),
            Self::OpenAlex(w) => write!(f, "{}:{w}", self.key()),
        }
    }
}

/// A name that owns its text: the registry's and the tables' form. Orders
/// by namespace in [`Key::ALL`] order, then by value, so a sorted list of
/// names is deterministic.
///
/// Equality, hash and order are [`Name`]'s, not the derive's, because the
/// text a publisher id owns is the spelling it was written in and two
/// spellings are one name. Everything that keys names — the registry's
/// table, the arena, the copy rule's `(source, native)` pairs, the
/// serving layer's maps — goes through them.
#[derive(Debug, Clone)]
pub enum OwnedName {
    #[allow(missing_docs)]
    Doi(Box<str>),
    #[allow(missing_docs)]
    Pmid(Pmid),
    #[allow(missing_docs)]
    Pmc(PmcId),
    #[allow(missing_docs)]
    OpenAlex(OpenAlexId),
    /// A publisher id and the registrant that issued it, the value in
    /// the spelling it was written in; see [`Name::Publisher`].
    Publisher(Registrant, Box<str>),
}

/// [`Name`]'s, over the canonical spelling: two spellings of one
/// article's PII are one name owned as they are borrowed.
impl PartialEq for OwnedName {
    fn eq(&self, other: &Self) -> bool {
        self.as_name() == other.as_name()
    }
}

impl Eq for OwnedName {}

impl Hash for OwnedName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_name().hash(state);
    }
}

impl Ord for OwnedName {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_name().cmp(&other.as_name())
    }
}

impl PartialOrd for OwnedName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
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
            Self::Publisher(registrant, value) => Name::Publisher(*registrant, value),
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

/// A field name as the identity pass keys its counts by: the same text
/// always the same `&'static str`. Field names are a closed, short set
/// — a source's identifier fields — so the leak is bounded.
#[must_use]
pub fn field_static(field: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static FIELDS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let fields = FIELDS.get_or_init(|| Mutex::new(HashSet::new()));
    let mut fields = fields
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(known) = fields.get(field) {
        return known;
    }
    let leaked: &'static str = Box::leak(field.to_owned().into_boxed_str());
    fields.insert(leaked);
    leaked
}

/// A field name as a small number, for what the identity pass keeps per
/// assertion: two bytes where a `&'static str` took sixteen, on some 10⁸
/// assertions (the merge's memory, step 2c, 2026-09-24). The names are a
/// closed, short set, numbered in the order a run first meets them and
/// kept in one table every run in the process shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Field(u16);

type FieldTable = (Vec<&'static str>, HashMap<&'static str, u16>);

fn fields() -> std::sync::MutexGuard<'static, FieldTable> {
    use std::sync::{Mutex, OnceLock};
    static FIELDS: OnceLock<Mutex<FieldTable>> = OnceLock::new();
    FIELDS
        .get_or_init(|| Mutex::new((Vec::new(), HashMap::new())))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Field {
    /// The field named `name`.
    ///
    /// # Panics
    ///
    /// More than 2^16 distinct field names in one process, which no source
    /// comes near.
    #[must_use]
    pub fn of(name: &str) -> Self {
        let mut table = fields();
        if let Some(&ix) = table.1.get(name) {
            return Self(ix);
        }
        let name = field_static(name);
        let ix = u16::try_from(table.0.len()).expect("fewer than 2^16 field names");
        table.0.push(name);
        table.1.insert(name, ix);
        Self(ix)
    }

    /// The field's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        fields().0[usize::from(self.0)]
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

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
    use model::{Doi, OpenAlexId, PmcId, Pmid};

    use super::{Name, OwnedName};

    fn registrant(prefix: &str) -> model::Registrant {
        model::Registrant::parse(prefix).unwrap()
    }

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

    #[test]
    fn a_publisher_id_is_named_by_its_registrant() {
        let elsevier = registrant("10.1016");
        let name = Name::Publisher(elsevier, "S0006-3495(25)00123-4");
        let owned = OwnedName::Publisher(elsevier, "S0006349525001234".into());
        assert_eq!(name.to_string(), "reg1016:S0006349525001234");
        assert_eq!(name.to_owned(), owned, "either spelling is the one name");
        assert_eq!(owned.as_name(), name, "either spelling is the one name");
        // Owning the text keeps the spelling; rendering it promotes one.
        let OwnedName::Publisher(_, written) = name.to_owned() else {
            unreachable!()
        };
        assert_eq!(&*written, "S0006-3495(25)00123-4", "stored as written");
        assert_eq!(
            OwnedName::parse("reg1016:S0006349525001234").unwrap(),
            owned
        );
        assert_eq!(owned.key(), ingest::Key::Publisher);
        assert_eq!(name.number(), None);
        assert_eq!(
            serde_json::to_string(&name).unwrap(),
            "\"reg1016:S0006349525001234\""
        );
        let bytes = postcard::to_allocvec(&name).unwrap();
        assert_eq!(
            postcard::from_bytes::<Name<'_>>(&bytes).unwrap().to_owned(),
            owned,
            "borrowed through postcard"
        );
        // The namespace scopes the value: one string, two publishers,
        // two names, and the order is by registrant then value.
        let elsewhere = OwnedName::Publisher(registrant("10.5555"), "S0006349525001234".into());
        assert_ne!(owned, elsewhere);
        assert!(owned < elsewhere);
        assert!(
            OwnedName::Doi("10.1000/x".into()) < owned,
            "namespace order"
        );
    }

    #[test]
    fn parse_refuses_a_namespace_shaped_like_a_registrant() {
        // The whole point of `reg1016`: nothing in the merged layer may
        // read as a DOI (decision log, 2026-09-19).
        for text in [
            "10.1016:S0006349525001234",
            "1016:S0006349525001234",
            "10.1053:S0006349525001234",
        ] {
            let err = OwnedName::parse(text).unwrap_err();
            assert!(err.reason.contains("never a registrant"), "{text}: {err}");
        }
        // And a namespace that is not a registrant at all.
        for text in [
            "reg:x",           // no digits
            "reg123:x",        // three digits
            "reg0123:x",       // a leading zero
            "reg1234567890:x", // ten digits
            "regabc:x",        // not digits
            "publisher:x",     // the key names the family, not a scope
        ] {
            assert!(OwnedName::parse(text).is_err(), "{text}");
        }
        // A value the namespace rewrites reads back in the spelling the
        // line was written in — which is not what an uppercase DOI does,
        // and deliberately: a DOI's canonical text is the model's and
        // fixed, a publisher id's is ours and changes with the list.
        let punctuated = OwnedName::parse("reg1016:S0006-3495(25)00123-4").unwrap();
        assert_eq!(
            punctuated,
            OwnedName::parse("reg1016:S0006349525001234").unwrap(),
            "one name, whichever spelling the line carried"
        );
        assert_eq!(punctuated.to_string(), "reg1016:S0006349525001234");
        // Under a registrant that is not Elsevier's, that same text is
        // its own name, because nothing rewrites it.
        assert!(OwnedName::parse("reg5555:S0006-3495(25)00123-4").is_ok());
        // And what is not an identifier does not become one by being
        // written as a name.
        for text in [
            "reg1016:",
            "reg1016:320",
            "reg1016:e112",
            "reg1016:10.1016/x",
        ] {
            assert!(OwnedName::parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn publisher_names_hash_by_their_canonical_spelling() {
        use std::collections::HashSet;
        let elsevier = Doi::parse("10.1016/x").unwrap().registrant();
        let mut set = HashSet::new();
        set.insert(Name::Publisher(elsevier, "S0006-3495(25)00123-4"));
        assert!(set.contains(&Name::Publisher(elsevier, "S0006349525001234")));
        assert!(!set.contains(&Name::Publisher(
            Doi::parse("10.5555/x").unwrap().registrant(),
            "S0006349525001234"
        )));
    }

    /// The spellings `data/merged/registry.jsonl` already holds, taken
    /// from it on 2026-09-19: 39,896 distinct `reg*` names across 338
    /// registrant namespaces, of which 183 are under 10.1006 and 10.1074
    /// — 99 punctuated, 84 bare — because those two are off
    /// `publisher::ELSEVIER` and both spellings are two names there. All
    /// of them are sealed. Every one must keep parsing, in the spelling
    /// its line carries, whatever the list says later; that is the whole
    /// point of the change, and `just report` reads all 39,896 of them.
    #[test]
    fn every_spelling_the_registry_already_sealed_still_parses() {
        // Under 10.1006 and 10.1074 the registry holds both spellings of
        // one article — 19 such pairs under the first, 57 under the
        // second — the first of these being the article the surveys
        // argued from (10.1006/jmbi.1998.2346) and the second the one the
        // merge of 2026-09-19 actually died on.
        let pairs = [
            ("reg1006", "S0022-2836(98)92346-4", "S0022283698923464"),
            ("reg1006", "S0003-2697(96)90292-4", "S0003269796902924"),
            ("reg1006", "S0022-2836(00)93729-X", "S002228360093729X"),
            ("reg1074", "S0021-9258(17)48553-X", "S002192581748553X"),
            ("reg1074", "S1535-9476(20)33724-5", "S1535947620337245"),
        ];
        for (namespace, punctuated, bare) in pairs {
            for value in [punctuated, bare] {
                let text = format!("{namespace}:{value}");
                let name = OwnedName::parse(&text).unwrap_or_else(|e| panic!("{e}"));
                assert_eq!(name.to_string(), text, "sealed as written");
            }
            // Off the list, so still two names — which is why the
            // registry holds both. Adding the registrant makes them one
            // and drops no line: the older spelling still reads.
            assert_ne!(
                OwnedName::parse(&format!("{namespace}:{punctuated}")).unwrap(),
                OwnedName::parse(&format!("{namespace}:{bare}")).unwrap(),
            );
        }
        // What those same values become the day their registrant joins
        // the list: a punctuated value under a registrant that rewrites
        // it. Before this change that was `"not canonical"` and the
        // merge stopped reading the registry at line 5,310,863.
        for (_, punctuated, bare) in pairs {
            let sealed = OwnedName::parse(&format!("reg1016:{punctuated}")).unwrap();
            assert_eq!(
                sealed,
                OwnedName::parse(&format!("reg1016:{bare}")).unwrap(),
                "{punctuated}: one name once the list rewrites it"
            );
            assert_eq!(sealed.to_string(), format!("reg1016:{bare}"), "promoted");
        }
        // And the shapes the other 337 namespaces hold, which no list
        // touches: they are identifiers because a publisher said so.
        for text in [
            "reg1016:S8756328226000396",
            "reg1016:a93276",
            "reg9778:8/3/E585",
            "reg7326:139/5_Part_2/437",
            "reg4103:ni_2022_70_1_270_338701",
            "reg5694:man10966_fm",
            "reg1515:/j/folmed.2016.58.issue-1/folmed-2016-0005/folmed-2016-0005.xml",
        ] {
            assert_eq!(OwnedName::parse(text).unwrap().to_string(), text);
        }
    }

    #[test]
    fn equal_names_sort_equal() {
        use std::collections::HashSet;
        let elsevier = registrant("10.1016");
        let punctuated = OwnedName::Publisher(elsevier, "S0006-3495(25)00123-4".into());
        let bare = OwnedName::Publisher(elsevier, "S0006349525001234".into());
        // Equal under `Eq`, so equal under `Ord` — anything else and a
        // sorted-then-deduplicated list keeps two of what a map keeps one
        // of, and the registry and the merged rows disagree.
        assert_eq!(punctuated, bare);
        assert_eq!(punctuated.cmp(&bare), std::cmp::Ordering::Equal);
        assert_eq!(
            punctuated.as_name().cmp(&bare.as_name()),
            std::cmp::Ordering::Equal
        );
        let mut both = vec![punctuated.clone(), bare.clone()];
        both.sort();
        both.dedup();
        assert_eq!(both.len(), 1, "one name, one entry");
        assert_eq!(
            HashSet::from([punctuated, bare]).len(),
            1,
            "and one key in a set"
        );
        // The order is still namespace first, then registrant, then
        // value — `Key::ALL`'s order, which is `Key`'s own.
        let mut sorted = [
            OwnedName::parse("reg1016:S0006349525001234").unwrap(),
            OwnedName::parse("doi:10.1000/x").unwrap(),
            OwnedName::parse("openalex:W6").unwrap(),
            OwnedName::parse("pmc:PMC45").unwrap(),
            OwnedName::parse("pmid:123").unwrap(),
            OwnedName::parse("reg1006:S0022-2836(98)92346-4").unwrap(),
        ];
        sorted.sort();
        assert_eq!(
            sorted.map(|n| n.to_string()),
            [
                "doi:10.1000/x",
                "pmid:123",
                "pmc:PMC45",
                "openalex:W6",
                "reg1006:S0022-2836(98)92346-4",
                "reg1016:S0006349525001234",
            ]
        );
    }

    #[test]
    fn field_names_intern_to_one_static() {
        let a = super::field_static("ids.doi");
        let b = super::field_static(&String::from("ids.doi"));
        assert!(std::ptr::eq(a, b));
        assert_eq!(super::field_static("pmid"), "pmid");
    }
}
