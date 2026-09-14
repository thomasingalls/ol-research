//! The streaming parser: quick-xml events in, DTD-shaped records out.
//!
//! Zero-copy by construction. Every [`Text`] and [`RawXml`] in a record is a
//! slice of the input string, and attribute values are re-borrowed from the
//! input rather than from quick-xml's event — its attribute iterator is tied
//! to the event, not the input, so values read through it could not outlive
//! one loop iteration without a copy. Only a resolved character or entity
//! reference allocates.
//!
//! Every element parser has the same shape: `enter`, handle attributes,
//! loop over children with one arm per element the DTD allows, build the
//! record with cardinality checks, `leave`. Anything else is an error at a
//! byte offset, on an element path, with the current PMID.

use std::borrow::Cow;

use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::escape;
use quick_xml::events::attributes::Attributes;
use quick_xml::events::{BytesEnd, BytesStart, Event};

use crate::error::{Error, ErrorKind};
use crate::record::{Date, Pmid, Record};
use crate::text::{RawXml, Text};

/// Stores `$value` into the `Option` `$slot`; a second occurrence is an error.
macro_rules! set_once {
    ($p:expr, $slot:expr, $name:literal, $value:expr) => {{
        let value = $value;
        if $slot.is_some() {
            return Err($p.error(ErrorKind::DuplicateElement($name)));
        }
        $slot = Some(value);
    }};
}

mod article;
mod citation;
mod data;

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// A start tag, re-borrowed from the input. A view, so it is `Copy`.
#[derive(Clone, Copy)]
pub(crate) struct Tag<'a> {
    /// Everything between `<` and `>` (or `/>`): `ISSN IssnType="Print"`.
    content: &'a str,
    name_len: usize,
    /// Self-closing, so the element has no content.
    empty: bool,
}

impl<'a> Tag<'a> {
    pub(crate) fn name(&self) -> &'a str {
        &self.content[..self.name_len]
    }

    fn attributes(&self) -> Attributes<'a> {
        Attributes::new(self.content, self.name_len)
    }
}

/// What follows inside an element: another element, or its end.
pub(crate) enum Child<'a> {
    Elem(Tag<'a>),
    End,
}

#[derive(Debug)]
pub(crate) struct Parser<'a> {
    input: &'a str,
    reader: Reader<&'a [u8]>,
    path: Vec<&'a str>,
    pmid: Option<Text<'a>>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        // quick-xml skips a BOM; skip it here too so offsets agree.
        let input = input.strip_prefix('\u{feff}').unwrap_or(input);
        Self {
            input,
            reader: Reader::from_str(input),
            path: Vec::with_capacity(16),
            pmid: None,
        }
    }

    pub(crate) fn error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            position: self.reader.buffer_position(),
            path: self.path.join("/"),
            pmid: self.pmid.as_deref().map(str::to_owned),
        }
    }

    pub(crate) fn enter(&mut self, tag: &Tag<'a>) {
        self.path.push(tag.name());
    }

    pub(crate) fn leave(&mut self) {
        self.path.pop();
    }

    pub(crate) fn set_pmid(&mut self, pmid: &Text<'a>) {
        self.pmid = Some(pmid.clone());
    }

    fn position(&self) -> usize {
        offset(self.reader.buffer_position())
    }

    /// Re-borrows a just-read start tag from the input: the reader stands
    /// right after `>`, and the event holds exactly the bytes between `<`
    /// and `>` (or `/>`).
    fn tag(&self, start: &BytesStart<'_>, empty: bool) -> Tag<'a> {
        let end = self.position() - 1 - usize::from(empty);
        let begin = end - start.len();
        let content = &self.input[begin..end];
        debug_assert_eq!(content, &**start, "tag span arithmetic");
        Tag {
            content,
            name_len: start.name().as_ref().len(),
            empty,
        }
    }

    /// The next child of `tag`, or `End` at its close.
    pub(crate) fn next_child(&mut self, tag: &Tag<'a>) -> Result<Child<'a>> {
        if tag.empty {
            return Ok(Child::End);
        }
        self.next_event_child()
    }

    fn next_event_child(&mut self) -> Result<Child<'a>> {
        loop {
            match self.reader.read_event() {
                Ok(Event::Start(s)) => return Ok(Child::Elem(self.tag(&s, false))),
                Ok(Event::Empty(s)) => return Ok(Child::Elem(self.tag(&s, true))),
                Ok(Event::End(_)) => return Ok(Child::End),
                Ok(Event::Comment(_) | Event::PI(_)) => {}
                Ok(Event::Text(t)) => {
                    if !t.trim().is_empty() {
                        return Err(self.error(ErrorKind::UnexpectedText(excerpt(&t))));
                    }
                }
                Ok(Event::CData(c)) => {
                    return Err(self.error(ErrorKind::UnexpectedText(excerpt(&c.into_inner()))));
                }
                Ok(Event::GeneralRef(r)) => {
                    return Err(self.error(ErrorKind::UnexpectedText(format!("&{};", &*r))));
                }
                Ok(Event::Decl(_)) => {
                    return Err(self.error(ErrorKind::UnexpectedElement("?xml".to_owned())));
                }
                Ok(Event::DocType(_)) => {
                    return Err(self.error(ErrorKind::UnexpectedElement("!DOCTYPE".to_owned())));
                }
                Ok(Event::Eof) => return Err(self.error(ErrorKind::UnexpectedEof)),
                Err(e) => return Err(self.error(ErrorKind::Xml(e))),
            }
        }
    }

    /// The raw inner span of `tag`; leaves the reader after its end tag.
    fn raw(&mut self, tag: &Tag<'a>) -> Result<&'a str> {
        if tag.empty {
            return Ok("");
        }
        let end = BytesEnd::new(tag.name());
        let span = self
            .reader
            .read_to_end(end.name())
            .map_err(|e| self.error(ErrorKind::Xml(e)))?;
        Ok(&self.input[offset(span.start)..offset(span.end)])
    }

    /// A `#PCDATA` element that takes no attributes.
    pub(crate) fn leaf(&mut self, tag: &Tag<'a>) -> Result<Text<'a>> {
        self.enter(tag);
        self.no_attributes(tag)?;
        let text = self.text(tag)?;
        self.leave();
        Ok(text)
    }

    /// The decoded content of a `#PCDATA` element whose attributes the
    /// caller has handled.
    pub(crate) fn text(&mut self, tag: &Tag<'a>) -> Result<Text<'a>> {
        let raw = self.raw(tag)?;
        if raw.contains('<') {
            return Err(self.error(ErrorKind::MarkupInLeaf));
        }
        escape::unescape(raw).map_err(|e| self.error(ErrorKind::Escape(e)))
    }

    /// A mixed-content element that takes no attributes.
    pub(crate) fn mixed(&mut self, tag: &Tag<'a>) -> Result<RawXml<'a>> {
        self.enter(tag);
        self.no_attributes(tag)?;
        let raw = self.markup(tag)?;
        self.leave();
        Ok(raw)
    }

    /// The raw content of a mixed-content element whose attributes the
    /// caller has handled.
    pub(crate) fn markup(&mut self, tag: &Tag<'a>) -> Result<RawXml<'a>> {
        self.raw(tag).map(RawXml::new)
    }

    /// Visits each attribute of `tag`, decoded, borrowed from the input.
    /// Duplicate attributes are rejected by quick-xml.
    pub(crate) fn each_attr<F>(&self, tag: &Tag<'a>, mut visit: F) -> Result<()>
    where
        F: FnMut(&'a str, Cow<'a, str>) -> Result<()>,
    {
        for attr in tag.attributes() {
            let attr = attr.map_err(|e| self.error(ErrorKind::Attr(e)))?;
            let name = attr.key.into_inner();
            let value = attr
                .normalized_value(XmlVersion::Explicit1_0)
                .map_err(|e| self.error(ErrorKind::Xml(e)))?;
            visit(name, value)?;
        }
        Ok(())
    }

    pub(crate) fn no_attributes(&self, tag: &Tag<'a>) -> Result<()> {
        self.each_attr(tag, |name, _| Err(self.unexpected_attr(name)))
    }

    pub(crate) fn unexpected(&self, tag: &Tag<'a>) -> Error {
        self.error(ErrorKind::UnexpectedElement(tag.name().to_owned()))
    }

    pub(crate) fn unexpected_attr(&self, name: &str) -> Error {
        self.error(ErrorKind::UnexpectedAttribute(name.to_owned()))
    }

    pub(crate) fn content_model(&self, detail: &'static str) -> Error {
        self.error(ErrorKind::ContentModel(detail))
    }

    pub(crate) fn required<T>(&self, name: &'static str, value: Option<T>) -> Result<T> {
        value.ok_or_else(|| self.error(ErrorKind::MissingElement(name)))
    }

    pub(crate) fn required_attr<T>(&self, name: &'static str, value: Option<T>) -> Result<T> {
        value.ok_or_else(|| self.error(ErrorKind::MissingAttribute(name)))
    }

    pub(crate) fn at_least_one<T>(&self, name: &'static str, items: Vec<T>) -> Result<Vec<T>> {
        if items.is_empty() {
            Err(self.error(ErrorKind::MissingElement(name)))
        } else {
            Ok(items)
        }
    }

    /// Parses an enumerated attribute; anything outside the vocabulary fails.
    pub(crate) fn enum_attr<T>(
        &self,
        name: &'static str,
        value: &str,
        parse: fn(&str) -> Option<T>,
    ) -> Result<T> {
        parse(value).ok_or_else(|| {
            self.error(ErrorKind::InvalidAttributeValue {
                name,
                value: value.to_owned(),
            })
        })
    }

    /// Reads past the prologue to the `PubmedArticleSet` start tag.
    fn open_root(&mut self) -> Result<()> {
        loop {
            match self.reader.read_event() {
                Ok(Event::Decl(_) | Event::DocType(_) | Event::Comment(_) | Event::PI(_)) => {}
                Ok(Event::Text(t)) if t.trim().is_empty() => {}
                Ok(Event::Start(s)) => {
                    let tag = self.tag(&s, false);
                    if tag.name() != "PubmedArticleSet" {
                        return Err(self.unexpected(&tag));
                    }
                    self.no_attributes(&tag)?;
                    self.enter(&tag);
                    return Ok(());
                }
                Ok(Event::Empty(s)) => {
                    let tag = self.tag(&s, true);
                    return Err(self.unexpected(&tag));
                }
                Ok(Event::Eof) => return Err(self.error(ErrorKind::UnexpectedEof)),
                Ok(_) => {
                    return Err(self.error(ErrorKind::UnexpectedText("before root".to_owned())));
                }
                Err(e) => return Err(self.error(ErrorKind::Xml(e))),
            }
        }
    }

    /// After the root's end tag: only whitespace and comments may follow.
    fn expect_eof(&mut self) -> Result<()> {
        loop {
            match self.reader.read_event() {
                Ok(Event::Eof) => return Ok(()),
                Ok(Event::Comment(_) | Event::PI(_)) => {}
                Ok(Event::Text(t)) if t.trim().is_empty() => {}
                Ok(Event::Start(s) | Event::Empty(s)) => {
                    let tag = self.tag(&s, false);
                    return Err(self.unexpected(&tag));
                }
                Ok(_) => return Err(self.error(ErrorKind::UnexpectedText("after root".to_owned()))),
                Err(e) => return Err(self.error(ErrorKind::Xml(e))),
            }
        }
    }
}

fn offset(position: u64) -> usize {
    usize::try_from(position).unwrap_or(usize::MAX)
}

fn excerpt(text: &str) -> String {
    text.trim().chars().take(40).collect()
}

/// `PMID (#PCDATA)` with `Version CDATA #REQUIRED`.
pub(crate) fn pmid<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Pmid<'a>> {
    p.enter(&tag);
    let mut version = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "Version" => version = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let version = p.required_attr("Version", version)?;
    let value = p.text(&tag)?;
    p.leave();
    Ok(Pmid { version, value })
}

/// Collects `Year`, `Month`, `Day` children for the several date shapes.
#[derive(Default)]
pub(crate) struct DateSlots<'a> {
    year: Option<Text<'a>>,
    month: Option<Text<'a>>,
    day: Option<Text<'a>>,
}

impl<'a> DateSlots<'a> {
    /// Consumes `tag` if it is one of the three; hands it back otherwise.
    pub(crate) fn take(&mut self, p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Option<Tag<'a>>> {
        match tag.name() {
            "Year" => set_once!(p, self.year, "Year", p.leaf(&tag)?),
            "Month" => set_once!(p, self.month, "Month", p.leaf(&tag)?),
            "Day" => set_once!(p, self.day, "Day", p.leaf(&tag)?),
            _ => return Ok(Some(tag)),
        }
        Ok(None)
    }

    pub(crate) fn finish(self, p: &Parser<'a>) -> Result<Date<'a>> {
        Ok(Date {
            year: p.required("Year", self.year)?,
            month: p.required("Month", self.month)?,
            day: p.required("Day", self.day)?,
        })
    }
}

/// `(Year, Month, Day)` with no attributes: `DateCompleted`, `DateRevised`.
pub(crate) fn date_ymd<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Date<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let mut slots = DateSlots::default();
    while let Child::Elem(t) = p.next_child(&tag)? {
        if let Some(t) = slots.take(p, t)? {
            return Err(p.unexpected(&t));
        }
    }
    let date = slots.finish(p)?;
    p.leave();
    Ok(date)
}

/// A wrapper element with no attributes whose content is `child+`.
pub(crate) fn list<'a, T>(
    p: &mut Parser<'a>,
    tag: Tag<'a>,
    child: &'static str,
    parse: impl Fn(&mut Parser<'a>, Tag<'a>) -> Result<T>,
) -> Result<Vec<T>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let mut items = Vec::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        if t.name() == child {
            items.push(parse(p, t)?);
        } else {
            return Err(p.unexpected(&t));
        }
    }
    let items = p.at_least_one(child, items)?;
    p.leave();
    Ok(items)
}

/// Iterates the records of one `PubmedArticleSet` document, borrowing each
/// from the input.
#[derive(Debug)]
pub struct ArticleSet<'a> {
    parser: Parser<'a>,
    state: State,
}

#[derive(Debug)]
enum State {
    Prologue,
    Body,
    Done,
}

impl<'a> ArticleSet<'a> {
    /// Prepares to read `input`, which must be one whole document.
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            parser: Parser::new(input),
            state: State::Prologue,
        }
    }

    /// The next record, or `None` after the root element closes cleanly.
    ///
    /// # Errors
    ///
    /// The first structural disagreement with the DTD, or malformed XML.
    /// After an error the set is exhausted; there is no recovery by design.
    pub fn next_record(&mut self) -> std::result::Result<Option<Record<'a>>, Error> {
        match self.state {
            State::Done => return Ok(None),
            State::Prologue => {
                self.parser.open_root()?;
                self.state = State::Body;
            }
            State::Body => {}
        }
        self.parser.pmid = None;
        let p = &mut self.parser;
        match p.next_event_child()? {
            Child::Elem(tag) => match tag.name() {
                "PubmedArticle" => Ok(Some(Record::Article(citation::pubmed_article(p, tag)?))),
                "DeleteCitation" => {
                    Ok(Some(Record::DeleteCitation(data::delete_citation(p, tag)?)))
                }
                "PubmedBookArticle" => Err(p.error(ErrorKind::Unsupported("PubmedBookArticle"))),
                _ => Err(p.unexpected(&tag)),
            },
            Child::End => {
                p.leave();
                p.expect_eof()?;
                self.state = State::Done;
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::many_single_char_names)] // element names in tiny documents
mod tests {
    use super::{ArticleSet, Child, ErrorKind, Parser};

    #[test]
    fn tags_are_reborrowed_from_the_input_with_attributes() {
        let input = "<r><a x=\"1 &amp; 2\" y='z'><b/><c k=\"v\" /></a></r>";
        let mut p = Parser::new(input);
        let Child::Elem(r) = p.next_event_child().unwrap() else {
            panic!()
        };
        assert_eq!(r.name(), "r");
        let Child::Elem(a) = p.next_child(&r).unwrap() else {
            panic!()
        };
        assert_eq!(a.name(), "a");
        let mut seen = Vec::new();
        p.each_attr(&a, |n, v| {
            // Borrowed from `input`, not from the event: still usable later.
            seen.push((n, v));
            Ok(())
        })
        .unwrap();
        assert_eq!(seen[0].0, "x");
        assert_eq!(&*seen[0].1, "1 & 2");
        assert!(
            matches!(seen[0].1, std::borrow::Cow::Owned(_)),
            "reference resolved"
        );
        assert_eq!(&*seen[1].1, "z");
        assert!(matches!(seen[1].1, std::borrow::Cow::Borrowed(_)));
        let Child::Elem(b) = p.next_child(&a).unwrap() else {
            panic!()
        };
        assert!(b.empty);
        assert_eq!(b.name(), "b");
        assert!(matches!(p.next_child(&b).unwrap(), Child::End));
        let Child::Elem(c) = p.next_child(&a).unwrap() else {
            panic!()
        };
        assert!(c.empty);
        assert_eq!(c.name(), "c");
        p.each_attr(&c, |n, v| {
            assert_eq!((n, &*v), ("k", "v"));
            Ok(())
        })
        .unwrap();
        assert!(matches!(p.next_child(&a).unwrap(), Child::End));
    }

    #[test]
    fn leaf_text_borrows_unless_a_reference_is_resolved() {
        let input = "<r><a>plain</a><b>x &lt; y</b><c/><d>a<i>b</i></d></r>";
        let mut p = Parser::new(input);
        let Child::Elem(r) = p.next_event_child().unwrap() else {
            panic!()
        };
        let Child::Elem(a) = p.next_child(&r).unwrap() else {
            panic!()
        };
        let a = p.leaf(&a).unwrap();
        assert!(matches!(a, std::borrow::Cow::Borrowed("plain")));
        let Child::Elem(b) = p.next_child(&r).unwrap() else {
            panic!()
        };
        let b = p.leaf(&b).unwrap();
        assert_eq!(&*b, "x < y");
        assert!(matches!(b, std::borrow::Cow::Owned(_)));
        let Child::Elem(c) = p.next_child(&r).unwrap() else {
            panic!()
        };
        assert_eq!(&*p.leaf(&c).unwrap(), "");
        let Child::Elem(d) = p.next_child(&r).unwrap() else {
            panic!()
        };
        assert!(matches!(
            p.leaf(&d).unwrap_err().kind,
            ErrorKind::MarkupInLeaf
        ));
    }

    #[test]
    fn mixed_content_is_kept_raw() {
        let input =
            "<r><t>a &amp; <i>b<sub>2</sub></i> <mml:math><mml:mi>x</mml:mi></mml:math></t></r>";
        let mut p = Parser::new(input);
        let Child::Elem(r) = p.next_event_child().unwrap() else {
            panic!()
        };
        let Child::Elem(t) = p.next_child(&r).unwrap() else {
            panic!()
        };
        let raw = p.mixed(&t).unwrap();
        assert_eq!(
            raw.as_str(),
            "a &amp; <i>b<sub>2</sub></i> <mml:math><mml:mi>x</mml:mi></mml:math>"
        );
        assert!(matches!(p.next_child(&r).unwrap(), Child::End));
    }

    #[test]
    fn root_must_be_an_article_set() {
        let mut set = ArticleSet::new("<?xml version=\"1.0\"?>\n<!DOCTYPE x>\n<Other/>");
        let err = set.next_record().unwrap_err();
        assert!(matches!(err.kind, ErrorKind::UnexpectedElement(ref n) if n == "Other"));
        let mut set = ArticleSet::new("<PubmedArticleSet></PubmedArticleSet>");
        assert!(set.next_record().unwrap().is_none());
        assert!(set.next_record().unwrap().is_none());
    }

    #[test]
    fn book_articles_are_not_modelled_yet() {
        let mut set = ArticleSet::new("<PubmedArticleSet><PubmedBookArticle/></PubmedArticleSet>");
        let err = set.next_record().unwrap_err();
        assert!(matches!(
            err.kind,
            ErrorKind::Unsupported("PubmedBookArticle")
        ));
        assert_eq!(err.path, "PubmedArticleSet");
    }
}
