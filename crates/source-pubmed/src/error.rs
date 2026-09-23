//! Why a file failed to parse, precisely enough to act on without re-running.

use std::fmt;

/// A parse failure: what, where in the file, where in the record, which record.
#[derive(Debug)]
pub struct Error {
    /// What went wrong.
    pub kind: ErrorKind,
    /// Byte offset into the decompressed file.
    pub position: u64,
    /// Element path from the root, e.g. `PubmedArticle/MedlineCitation/Article`.
    pub path: String,
    /// The record's PMID, when it had been read before the failure.
    pub pmid: Option<String>,
}

/// The structural ways the input can disagree with the DTD, plus the
/// underlying XML errors.
#[derive(Debug)]
pub enum ErrorKind {
    /// Malformed XML, per quick-xml.
    Xml(quick_xml::Error),
    /// A character or entity reference that cannot be resolved.
    Escape(quick_xml::escape::EscapeError),
    /// A malformed or duplicated attribute.
    Attr(quick_xml::events::attributes::AttrError),
    /// An element the DTD does not allow here.
    UnexpectedElement(String),
    /// Non-whitespace text where the DTD allows only elements.
    UnexpectedText(String),
    /// The file ended inside a record.
    UnexpectedEof,
    /// A required element is absent.
    MissingElement(&'static str),
    /// An element allowed at most once appeared again.
    DuplicateElement(&'static str),
    /// An attribute the DTD does not declare.
    UnexpectedAttribute(String),
    /// A `#REQUIRED` attribute is absent.
    MissingAttribute(&'static str),
    /// An attribute value outside its declared vocabulary.
    InvalidAttributeValue {
        /// The attribute.
        name: &'static str,
        /// What the file said.
        value: String,
    },
    /// Child markup inside a `#PCDATA` element.
    MarkupInLeaf,
    /// Children present in a combination the content model forbids.
    ContentModel(&'static str),
    /// Valid per the DTD, not modelled yet.
    Unsupported(&'static str),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Xml(e) => write!(f, "malformed xml: {e}"),
            Self::Escape(e) => write!(f, "unresolvable reference: {e}"),
            Self::Attr(e) => write!(f, "malformed attribute: {e}"),
            Self::UnexpectedElement(name) => write!(f, "unexpected element <{name}>"),
            Self::UnexpectedText(excerpt) => write!(f, "unexpected text {excerpt:?}"),
            Self::UnexpectedEof => f.write_str("unexpected end of file"),
            Self::MissingElement(name) => write!(f, "missing required <{name}>"),
            Self::DuplicateElement(name) => write!(f, "<{name}> appears more than once"),
            Self::UnexpectedAttribute(name) => write!(f, "unexpected attribute {name}"),
            Self::MissingAttribute(name) => write!(f, "missing required attribute {name}"),
            Self::InvalidAttributeValue { name, value } => {
                write!(
                    f,
                    "attribute {name} has value {value:?} outside its vocabulary"
                )
            }
            Self::MarkupInLeaf => f.write_str("markup inside a #PCDATA element"),
            Self::ContentModel(detail) => write!(f, "content model violated: {detail}"),
            Self::Unsupported(what) => write!(f, "not modelled yet: {what}"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at byte {} in {}",
            self.kind, self.position, self.path
        )?;
        if let Some(pmid) = &self.pmid {
            write!(f, " (PMID {pmid})")?;
        }
        Ok(())
    }
}

impl ingest::source::Signature for Error {
    /// The element path and the disagreement, without the byte offset or
    /// the PMID: the same wart in ten thousand records is one line of a
    /// survey.
    fn signature(&self) -> String {
        format!("{}: {}", self.path, self.kind)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::Xml(e) => Some(e),
            ErrorKind::Escape(e) => Some(e),
            ErrorKind::Attr(e) => Some(e),
            _ => None,
        }
    }
}
