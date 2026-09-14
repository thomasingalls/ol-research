//! PubMed baseline and update XML: the native record types and a strict,
//! zero-copy streaming parser.
//!
//! The native types in [`record`] mirror the DTD the files declare
//! (`pubmed_250101.dtd`, kept verbatim in `fixtures/`), which is PubMed's
//! normative description of its own serializer (plan §4a). The mapping is
//! mechanical: a `#PCDATA` leaf is a [`Text`]; mixed content —
//! `(%text; | mml:math)*` — is a [`RawXml`] span kept exactly as serialized;
//! an enumerated attribute is an enum in [`attr`]; cardinality is enforced.
//! Order of children is not enforced yet.
//!
//! Nothing is normalized in [`record`]. Dates stay text, identifiers stay
//! text; [`normalize`] turns them into `model` types and is where the strict
//! value rules bite. This layer's strictness is structural: an element, attribute
//! or attribute value the DTD does not allow stops the run with a byte
//! offset, an element path and the record's PMID.

pub mod attr;
pub mod error;
/// Reading raw files; shared with the other sources.
pub use ingest::gz;
pub mod normalize;
mod parse;
pub mod record;
pub mod text;

pub use error::{Error, ErrorKind};
pub use parse::ArticleSet;
pub use text::{RawXml, Text};
