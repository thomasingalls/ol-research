//! A query string as the parameters a route declares, and the one place
//! those names are written.
//!
//! Each route named its parameters three times until 2026-09-19: the
//! list `only()` refused anything outside, a `one()` / `count()` /
//! `flag()` call per parameter, and the literal strings the browser
//! client built its URLs from. Three lists that had to agree by hand,
//! and one evening they did not — the client sent `page` where the route
//! reads `page_size`, which the server refused correctly and the client
//! could not have been compiled against.
//!
//! So a route declares a struct instead ([`crate::http`]) and serde
//! fills it in from the pairs. The struct's fields are the whole of what
//! the route takes: serde hands them to [`deserialize_struct`] as
//! `fields`, so a parameter the route does not read is refused without
//! anyone listing it; the field's type is the `one()` / `count()` /
//! `flag()` distinction, declared rather than called; and the same
//! struct generates the TypeScript the client builds its URLs from
//! ([`crate::ts`]), so the third list is the first one again.
//!
//! [`deserialize_struct`]: serde::Deserializer::deserialize_struct
//!
//! What a struct cannot carry is a range. The caps stay in
//! [`crate::service::bounded`], which both transports call and which
//! labels its refusal with the name the transport spells the parameter
//! by — `facet_values` here where the protocol says `facets`.
//!
//! Every refusal below is worded as the hand-written one was. serde's
//! own prose reaches no client: the unknown field, the missing field,
//! the unknown enum variant and the value of the wrong shape are each
//! caught here, where the parameter's own name is still in hand and the
//! message can name it. `deserialize_struct` is the only entry point,
//! and it walks the declared fields itself, so serde is never in a
//! position to raise one of its own.
//!
//! An empty value — `?work=` — is refused wherever a value is parsed and
//! wherever one is required, and is the empty string for an optional
//! one. That is the line `one()` drew against `at_most_one()` and
//! `every()`, and it is the line `cursor` needs: an empty cursor is the
//! start of a list, which is what the protocol means by it, where an
//! empty `work` is a client that built a URL wrong.

use std::fmt;
use std::vec;

use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use serde::de::{
    self, DeserializeOwned, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};
use serde::forward_to_deserialize_any;
use tonic::Status;

use crate::http::Refusal;

/// The parameters a route takes, read off the query string into the
/// struct the route declares. Every route's first extractor, so that a
/// misspelled parameter is named before whatever it left missing.
#[derive(Debug)]
pub(crate) struct Params<T>(pub(crate) T);

impl<S, T> FromRequestParts<S> for Params<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Refusal;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // As the pairs it was written with, repeats and all: an
        // expansion names its seeds one `seeds` at a time, since a name
        // may hold any punctuation a separator could use, and
        // `serde_urlencoded` — which every other axum `Query` goes
        // through — has no shape for a key given twice.
        let Query(pairs) = Query::<Vec<(String, String)>>::from_request_parts(parts, state)
            .await
            .map_err(|e| Refusal::from(Status::invalid_argument(e.body_text())))?;
        T::deserialize(Written(&pairs))
            .map(Params)
            .map_err(Into::into)
    }
}

/// Why a query string was refused, in the words the client is told.
#[derive(Debug)]
pub(crate) struct Invalid(String);

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Invalid {}

/// serde raises its own errors through this — an unreachable path, as
/// the module says, and one worth seeing plainly if it ever is reached.
impl de::Error for Invalid {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self(message.to_string())
    }
}

impl From<Invalid> for Refusal {
    fn from(invalid: Invalid) -> Self {
        Self::from(Status::invalid_argument(invalid.0))
    }
}

/// A query string as written: the pairs, in order, repeats kept.
struct Written<'a>(&'a [(String, String)]);

impl<'de> de::Deserializer<'de> for Written<'_> {
    type Error = Invalid;

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Invalid> {
        // `only()`, derived: the fields the route declared are the whole
        // of what it reads, and anything else is refused rather than
        // ignored — a misspelled parameter is otherwise a wrong answer
        // given confidently.
        for (key, _) in self.0 {
            if !fields.contains(&key.as_str()) {
                return Err(Invalid(format!(
                    "{key:?} is not a parameter of this route; it takes {}",
                    if fields.is_empty() {
                        "none".to_owned()
                    } else {
                        fields.join(", ")
                    }
                )));
            }
        }
        visitor.visit_map(Declared {
            written: self.0,
            fields,
            at: 0,
        })
    }

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Invalid> {
        Err(Invalid(
            "a query string is the parameters of a route, which is a struct".to_owned(),
        ))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}

/// The declared fields, each in turn, whether the query string carried
/// it or not. Walking the declaration and not the query string is what
/// keeps every refusal below this line one of ours: serde is never told
/// of a key it does not know, nor left to find a field missing.
struct Declared<'a> {
    written: &'a [(String, String)],
    fields: &'static [&'static str],
    at: usize,
}

impl<'de> MapAccess<'de> for Declared<'_> {
    type Error = Invalid;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Invalid> {
        let Some(field) = self.fields.get(self.at) else {
            return Ok(None);
        };
        self.at += 1;
        seed.deserialize(IntoDeserializer::<Invalid>::into_deserializer(*field))
            .map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Invalid> {
        let key = self.fields[self.at - 1];
        seed.deserialize(Given {
            key,
            values: self
                .written
                .iter()
                .filter(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
                .collect(),
            empty_is_a_value: false,
        })
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.fields.len() - self.at)
    }
}

/// What one parameter was given: its name, so a refusal can say it, and
/// every value written for it, in the order they were written.
struct Given<'a> {
    key: &'static str,
    values: Vec<&'a str>,
    empty_is_a_value: bool,
}

impl<'a> Given<'a> {
    /// The single value of a parameter that takes one.
    fn one(&self) -> Result<&'a str, Invalid> {
        match self.values[..] {
            [] => Err(Invalid(format!("{} is required", self.key))),
            [value] => Ok(value),
            _ => Err(Invalid(format!("{} was given more than once", self.key))),
        }
    }

    /// A count, in the width the field asked for: the same refusal
    /// `count()` gave, and the same one for a number too big to be the
    /// count it claims to be.
    fn count<T: std::str::FromStr>(&self) -> Result<T, Invalid> {
        let text = self.one()?;
        text.parse()
            .map_err(|_| Invalid(format!("{}={text:?} is not a count", self.key)))
    }
}

impl<'de> de::Deserializer<'de> for Given<'_> {
    type Error = Invalid;

    /// Absent is absent; present is present, even when what is present
    /// is empty — `hydrate=` is a flag that is neither `true` nor
    /// `false`, and is told so.
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Invalid> {
        if self.values.is_empty() {
            return visitor.visit_none();
        }
        visitor.visit_some(Given {
            empty_is_a_value: true,
            ..self
        })
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Invalid> {
        let value = self.one()?;
        if value.is_empty() && !self.empty_is_a_value {
            return Err(Invalid(format!("{} is empty", self.key)));
        }
        visitor.visit_str(value)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Invalid> {
        self.deserialize_str(visitor)
    }

    /// `true` and `false` and nothing else: `1` is a client guessing,
    /// and a flag guessed at is a whole field of an answer silently
    /// absent.
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Invalid> {
        match self.one()? {
            "true" => visitor.visit_bool(true),
            "false" => visitor.visit_bool(false),
            other => Err(Invalid(format!(
                "{}={other:?}; true or false expected",
                self.key
            ))),
        }
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Invalid> {
        visitor.visit_u32(self.count()?)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Invalid> {
        visitor.visit_u64(self.count()?)
    }

    /// A repeated parameter, every value in the order written. Empty is
    /// an empty list, which the route refuses in its own words —
    /// `0 seeds; one to 100 expected` says more than "seeds is
    /// required" would.
    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Invalid> {
        visitor.visit_seq(Each {
            key: self.key,
            values: self.values.into_iter(),
        })
    }

    /// One of the names the enum spells, checked here rather than left
    /// to serde, so the refusal names the parameter and lists what it
    /// takes: `walk="sideways"; both, references, citers expected`. The
    /// list is the enum's own variants, so it cannot fall behind them.
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Invalid> {
        let text = self.one()?;
        if !variants.contains(&text) {
            return Err(Invalid(format!(
                "{}={text:?}; {} expected",
                self.key,
                variants.join(", ")
            )));
        }
        visitor.visit_enum(Named { text })
    }

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Invalid> {
        Err(Invalid(format!(
            "{} is not a shape a query string carries",
            self.key
        )))
    }

    forward_to_deserialize_any! {
        i8 i16 i32 i64 i128 u8 u16 u128 f32 f64 char bytes byte_buf unit
        unit_struct newtype_struct tuple tuple_struct map struct identifier
        ignored_any
    }
}

/// Every value of a repeated parameter. Each is the value as written,
/// empty included, which is what `every()` handed the route.
struct Each<'a> {
    key: &'static str,
    values: vec::IntoIter<&'a str>,
}

impl<'de> SeqAccess<'de> for Each<'_> {
    type Error = Invalid;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Invalid> {
        let Some(value) = self.values.next() else {
            return Ok(None);
        };
        seed.deserialize(Given {
            key: self.key,
            values: vec![value],
            empty_is_a_value: true,
        })
        .map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.values.len())
    }
}

/// A name that is one of an enum's variants, already checked to be one.
struct Named<'a> {
    text: &'a str,
}

impl<'de> EnumAccess<'de> for Named<'_> {
    type Error = Invalid;
    type Variant = NoPayload;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, NoPayload), Invalid> {
        let variant =
            seed.deserialize(IntoDeserializer::<Invalid>::into_deserializer(self.text))?;
        Ok((variant, NoPayload))
    }
}

/// A variant is a name and never carries anything: a query string has
/// nowhere to put it. The three that would are unreachable — every enum
/// on this surface is a set of bare names — and say so rather than
/// inventing an empty value.
struct NoPayload;

impl<'de> VariantAccess<'de> for NoPayload {
    type Error = Invalid;

    fn unit_variant(self) -> Result<(), Invalid> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, _seed: T) -> Result<T::Value, Invalid> {
        Err(Invalid(
            "a query string carries a name, not a value".to_owned(),
        ))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value, Invalid> {
        Err(Invalid(
            "a query string carries a name, not a value".to_owned(),
        ))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Invalid> {
        Err(Invalid(
            "a query string carries a name, not a value".to_owned(),
        ))
    }
}
