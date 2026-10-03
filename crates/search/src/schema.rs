//! The mapping, provisional. The user has the opinions here; this is one
//! function to exercise the pipeline until those are written down, and
//! [`DESCRIPTION`] is what every report prints so nobody mistakes it for
//! a decision.
//!
//! What it does today: title, abstract, authors and venue as text through
//! Tantivy's default tokenizer — split on non-alphanumerics, lowercased,
//! no stemming, no stop words, terms over 40 bytes dropped; one facet
//! field with the paths `/year/<y>`, `/form/<form>`, `/genre/<genre>`,
//! `/language/<code>`, `/status/<flag>`, `/held_by/<source>` and
//! `/access/<open|closed>`; the id as a stored, fast integer; the title
//! stored, so a hit can be shown without the works index.
//!
//! A treatment's index (tokenizer plan T1.2) is the same mapping with the
//! four text fields through [`crate::analyzer`] instead, under the name
//! its switches spell; the published index names none, and is as above.

use merge::Document;
use tantivy::schema::{
    FAST, FacetOptions, Field, FieldType, INDEXED, IndexRecordOption, STORED, Schema, TEXT,
    TextFieldIndexing, TextOptions,
};

use crate::analyzer::Switches;
use crate::error::Error;

/// What the report says the published mapping is: [`description`] with no
/// analyzer named.
pub const DESCRIPTION: &str = "provisional: title, abstract, authors, venue as text (default tokenizer: split, lowercase, no stemming); \
facets year, form, genre, language, status, held_by, access; stored id and title";

/// What the report says the mapping is, under `analyzer` when a treatment
/// names one.
#[must_use]
pub fn description(analyzer: Option<Switches>) -> String {
    match analyzer {
        None => DESCRIPTION.to_owned(),
        Some(switches) => format!(
            "provisional: title, abstract, authors, venue as text (analyzer {}, tokenizer plan D2–D6); \
             facets year, form, genre, language, status, held_by, access; stored id and title",
            switches.name()
        ),
    }
}

/// The field handles of the mapping.
#[derive(Debug, Clone, Copy)]
pub struct Fields {
    #[allow(missing_docs)]
    pub id: Field,
    #[allow(missing_docs)]
    pub title: Field,
    #[allow(missing_docs)]
    pub abstract_text: Field,
    #[allow(missing_docs)]
    pub authors: Field,
    #[allow(missing_docs)]
    pub venue: Field,
    #[allow(missing_docs)]
    pub facet: Field,
}

impl Fields {
    /// The text fields a query searches, in order.
    #[must_use]
    pub fn searched(&self) -> Vec<Field> {
        vec![self.title, self.abstract_text, self.authors, self.venue]
    }

    /// The text fields a citation's words are searched in: what a
    /// citation names a work by — its title, its authors, its journal —
    /// and not its abstract, which no citation carries and which shares
    /// words with everything (citation lookup plan, D3).
    #[must_use]
    pub fn cited(&self) -> Vec<Field> {
        vec![self.title, self.authors, self.venue]
    }
}

/// The provisional mapping: its text through Tantivy's `default`
/// tokenizer, or through `analyzer` under the name its switches spell —
/// indexed alike otherwise, with frequencies and positions.
#[must_use]
pub fn provisional(analyzer: Option<Switches>) -> (Schema, Fields) {
    let text = analyzer.map_or(TEXT, |switches| {
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(&switches.name())
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
    });
    let mut b = Schema::builder();
    let fields = Fields {
        id: b.add_u64_field("id", INDEXED | STORED | FAST),
        title: b.add_text_field("title", text.clone() | STORED),
        abstract_text: b.add_text_field("abstract", text.clone()),
        authors: b.add_text_field("authors", text.clone()),
        venue: b.add_text_field("venue", text),
        facet: b.add_facet_field("facet", FacetOptions::default()),
    };
    (b.build(), fields)
}

/// The name of the analyzer `field`'s text is split by, when it is
/// indexed text.
#[must_use]
pub fn analyzer_of(schema: &Schema, field: Field) -> Option<&str> {
    match schema.get_field_entry(field).field_type() {
        FieldType::Str(options) => options
            .get_indexing_options()
            .map(TextFieldIndexing::tokenizer),
        _ => None,
    }
}

/// The mapping's fields in an opened index's schema.
///
/// # Errors
///
/// A field the schema lacks: another mapping's index.
pub fn fields(schema: &Schema) -> Result<Fields, Error> {
    let get = |name: &str| {
        schema
            .get_field(name)
            .map_err(|_| Error::Mismatch(format!("the index has no field {name}")))
    };
    Ok(Fields {
        id: get("id")?,
        title: get("title")?,
        abstract_text: get("abstract")?,
        authors: get("authors")?,
        venue: get("venue")?,
        facet: get("facet")?,
    })
}

/// The facet roots the mapping fills.
pub const ROOTS: [&str; 7] = [
    "/year",
    "/form",
    "/genre",
    "/language",
    "/status",
    "/held_by",
    "/access",
];

/// A document's facet paths.
#[must_use]
pub fn facets(d: &Document) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(year) = d.year {
        paths.push(format!("/year/{year}"));
    }
    if let Some(form) = d.form {
        paths.push(format!("/form/{form}"));
    }
    if let Some(genre) = d.genre {
        paths.push(format!("/genre/{genre}"));
    }
    if let Some(language) = d.language {
        paths.push(format!("/language/{language}"));
    }
    for (flag, on) in [
        ("retracted", d.retracted),
        ("corrected", d.corrected),
        ("concern", d.concern),
    ] {
        if on {
            paths.push(format!("/status/{flag}"));
        }
    }
    for source in d.held_by.iter() {
        paths.push(format!("/held_by/{source}"));
    }
    if let Some(open) = d.access {
        paths.push(format!("/access/{}", if open { "open" } else { "closed" }));
    }
    paths
}
