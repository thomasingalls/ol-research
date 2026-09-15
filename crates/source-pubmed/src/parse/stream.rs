//! Streaming: one record at a time from a byte stream, each parsed from a
//! frame cut out of it, so a worker holds one record's text and never a
//! file's (plan §5).
//!
//! The framer is deliberately simple. It reads through the prologue to
//! the root's start tag, then cuts each record element at its own end tag
//! by name — `</PubmedArticle>` or `</DeleteCitation>` — and hands the
//! frame to the strict parser with its offset in the file, so errors say
//! file positions. Two facts make the cut exact: `<` cannot appear
//! unescaped in text, so an end tag inside a frame can only be markup;
//! and the DTD nests neither record element in itself. Whatever the
//! framer does not recognize goes to the parser as it is, so the error is
//! the parser's and the same the whole-document reader would give.

use std::io::{self, BufRead};
use std::ops::ControlFlow;

use ingest::frame::{ReadError, utf8};

use super::{Child, Parser, citation, data};
use crate::error::{Error, ErrorKind};
use crate::record::Record;

/// A frame buffer's starting size; it grows to the longest record.
const FRAME: usize = 256 * 1024;

const ROOT: &str = "PubmedArticleSet";

/// What the framer cut out.
enum Frame {
    /// A comment or processing instruction between records.
    Skip,
    /// A record element — or something the parser will reject.
    Element,
    /// An end tag at the top level: the root's, if the file is right.
    End,
}

/// The records of one `PubmedArticleSet` document read as a stream, each
/// handed to `each` borrowed from a frame buffer reused for the next.
///
/// # Errors
///
/// The stream's, or the first structural disagreement with the DTD, with
/// its byte offset in the file, element path and PMID.
pub fn records<R: BufRead, F>(mut input: R, mut each: F) -> Result<(), ReadError<Error>>
where
    F: FnMut(&Record<'_>) -> ControlFlow<()>,
{
    let mut buf = Vec::with_capacity(FRAME);
    let mut base = 0u64;
    prologue(&mut input, &mut buf)?;
    {
        let mut p = Parser::at(utf8(&buf)?, base);
        p.open_root().map_err(ReadError::Parse)?;
        exhausted(&p).map_err(ReadError::Parse)?;
    }
    loop {
        base += buf.len() as u64;
        buf.clear();
        match frame(&mut input, &mut buf)? {
            Frame::Skip => {}
            Frame::Element => {
                let mut p = Parser::at(utf8(&buf)?, base);
                p.path.push(ROOT);
                let record = match p.next_event_child().map_err(ReadError::Parse)? {
                    Child::Elem(tag) => match tag.name() {
                        "PubmedArticle" => Record::Article(
                            citation::pubmed_article(&mut p, tag).map_err(ReadError::Parse)?,
                        ),
                        "DeleteCitation" => Record::DeleteCitation(
                            data::delete_citation(&mut p, tag).map_err(ReadError::Parse)?,
                        ),
                        "PubmedBookArticle" => {
                            return Err(ReadError::Parse(
                                p.error(ErrorKind::Unsupported("PubmedBookArticle")),
                            ));
                        }
                        _ => return Err(ReadError::Parse(p.unexpected(&tag))),
                    },
                    Child::End => {
                        return Err(ReadError::Parse(p.error(ErrorKind::UnexpectedText(
                            "an end tag where a record should start".to_owned(),
                        ))));
                    }
                };
                exhausted(&p).map_err(ReadError::Parse)?;
                if each(&record).is_break() {
                    return Ok(());
                }
            }
            Frame::End => {
                return end(utf8(&buf)?, base).map_err(ReadError::Parse);
            }
        }
    }
}

/// The parser has read its whole frame: nothing trails the tag it stopped
/// after.
fn exhausted(p: &Parser<'_>) -> Result<(), Error> {
    if p.position() == p.input.len() {
        Ok(())
    } else {
        Err(p.error(ErrorKind::UnexpectedText(
            p.input[p.position()..].trim().chars().take(40).collect(),
        )))
    }
}

/// The last frame: the root's end tag, then only whitespace to the end.
fn end(text: &str, base: u64) -> Result<(), Error> {
    let error = |kind, at: usize| Error {
        kind,
        position: base + at as u64,
        path: ROOT.to_owned(),
        pmid: None,
    };
    let at = text.len() - text.trim_start().len();
    let tag = &text[at..];
    let Some(rest) = tag.strip_prefix("</PubmedArticleSet>") else {
        let name = tag.trim_start_matches("</");
        let name = name
            .split(['>', ' ', '\t', '\r', '\n'])
            .next()
            .unwrap_or(name);
        return Err(error(ErrorKind::UnexpectedElement(format!("/{name}")), at));
    };
    if !rest.trim().is_empty() {
        let at = text.len() - rest.trim_start().len();
        return Err(error(
            ErrorKind::UnexpectedText(rest.trim().chars().take(40).collect()),
            at,
        ));
    }
    Ok(())
}

/// Reads through the prologue — declaration, doctype, comments — up to
/// and including the first element's start tag, which the parser then
/// checks is the root's. A doctype with an internal subset is not
/// handled: a `>` inside its `[…]` would cut the frame short, and the
/// parser would say so.
fn prologue<R: BufRead>(input: &mut R, buf: &mut Vec<u8>) -> io::Result<()> {
    loop {
        let n = input.read_until(b'>', buf)?;
        if n == 0 {
            return Ok(());
        }
        match first_tag(&buf[buf.len() - n..]) {
            Some(tag) if tag.starts_with(b"!--") => {
                if !buf.ends_with(b"-->") {
                    read_through(input, buf, b"-->")?;
                }
            }
            Some(tag) if tag.starts_with(b"?") || tag.starts_with(b"!") => {}
            // An element's start tag, or text: the parser decides.
            Some(_) | None => return Ok(()),
        }
    }
}

/// Cuts the next frame from `input` into `buf`: a whole record element
/// when it starts with one the parser knows, otherwise just enough for the
/// parser to name what is wrong.
fn frame<R: BufRead>(input: &mut R, buf: &mut Vec<u8>) -> io::Result<Frame> {
    if input.read_until(b'>', buf)? == 0 {
        // The file ended before the root closed; an empty frame is the
        // parser's `UnexpectedEof`.
        return Ok(Frame::Element);
    }
    let Some(tag) = first_tag(buf) else {
        return Ok(Frame::Element);
    };
    if tag.starts_with(b"!--") {
        if !buf.ends_with(b"-->") {
            read_through(input, buf, b"-->")?;
        }
        return Ok(Frame::Skip);
    }
    if tag.starts_with(b"?") {
        if !buf.ends_with(b"?>") {
            read_through(input, buf, b"?>")?;
        }
        return Ok(Frame::Skip);
    }
    if tag.starts_with(b"/") {
        input.read_to_end(buf)?;
        return Ok(Frame::End);
    }
    if tag.ends_with(b"/") {
        // Self-closing: complete as it is; the parser will miss its children.
        return Ok(Frame::Element);
    }
    let name = tag.split(u8::is_ascii_whitespace).next().unwrap_or(tag);
    match name {
        b"PubmedArticle" => read_through(input, buf, b"</PubmedArticle>")?,
        b"DeleteCitation" => read_through(input, buf, b"</DeleteCitation>")?,
        // Unknown, or not modelled: the parser rejects the start tag itself.
        _ => {}
    }
    Ok(Frame::Element)
}

/// The tag a piece opens with — the bytes between its first `<` and the
/// `>` it was read up to — when only whitespace precedes it; `None` for
/// text.
fn first_tag(piece: &[u8]) -> Option<&[u8]> {
    let tag = piece.trim_ascii_start().strip_prefix(b"<")?;
    Some(tag.strip_suffix(b">").unwrap_or(tag))
}

/// Reads tag by tag until the buffer ends with `end`, or the input does.
fn read_through<R: BufRead>(input: &mut R, buf: &mut Vec<u8>, end: &[u8]) -> io::Result<()> {
    loop {
        if input.read_until(b'>', buf)? == 0 || buf.ends_with(end) {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::ops::ControlFlow;

    use ingest::ReadError;

    use super::records;
    use crate::error::{Error, ErrorKind};
    use crate::record::Record;

    /// The PMIDs of every record, deletions flattened.
    fn pmids(text: &str) -> Result<Vec<String>, Error> {
        let mut out = Vec::new();
        let result = records(Cursor::new(text), |r| {
            match r {
                Record::Article(a) => out.push(a.medline_citation.pmid.value.to_string()),
                Record::DeleteCitation(d) => {
                    out.extend(d.pmids.iter().map(|p| p.value.to_string()));
                }
            }
            ControlFlow::Continue(())
        });
        match result {
            Ok(()) => Ok(out),
            Err(ReadError::Parse(e)) => Err(e),
            Err(ReadError::Io(e)) => panic!("{e}"),
        }
    }

    #[test]
    fn deletions_comments_and_instructions_between_records() {
        let text = "<?xml version=\"1.0\"?>\n<!DOCTYPE PubmedArticleSet PUBLIC \"x\" \"y\">\n\
                    <PubmedArticleSet>\n<!-- a > b -->\n<?pi x?>\n\
                    <DeleteCitation><PMID Version=\"1\">1</PMID><PMID Version=\"1\">2</PMID></DeleteCitation>\n\
                    <DeleteCitation>\n  <PMID Version=\"1\">3</PMID>\n</DeleteCitation>\n\
                    </PubmedArticleSet>\n";
        assert_eq!(pmids(text).unwrap(), ["1", "2", "3"]);
        let mut seen = 0;
        records(Cursor::new(text), |_| {
            seen += 1;
            ControlFlow::Break(())
        })
        .unwrap();
        assert_eq!(seen, 1);
    }

    #[test]
    fn errors_carry_file_offsets_paths_and_the_parsers_verdict() {
        let err = pmids("<?xml version=\"1.0\"?>\n<!DOCTYPE x>\n<Other/>").unwrap_err();
        assert!(matches!(err.kind, ErrorKind::UnexpectedElement(ref n) if n == "Other"));

        let head = "<PubmedArticleSet>";
        let err = pmids(&format!("{head}<PubmedBookArticle/></PubmedArticleSet>")).unwrap_err();
        assert!(matches!(
            err.kind,
            ErrorKind::Unsupported("PubmedBookArticle")
        ));
        assert_eq!(err.path, "PubmedArticleSet");
        assert_eq!(
            err.position,
            "<PubmedArticleSet><PubmedBookArticle/>".len() as u64
        );

        let err = pmids(&format!("{head}<DeleteCitation/></PubmedArticleSet>")).unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::MissingElement("PMID")),
            "{err}"
        );

        let err = pmids(&format!(
            "{head}<DeleteCitation><PMID Version=\"1\">1</PMID></DeleteCitation></Other>"
        ))
        .unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::UnexpectedElement(ref n) if n == "/Other"),
            "{err}"
        );
        assert_eq!(
            err.position,
            "<PubmedArticleSet><DeleteCitation><PMID Version=\"1\">1</PMID></DeleteCitation>".len()
                as u64
        );

        let err = pmids(&format!("{head}</PubmedArticleSet>\ntrailing")).unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::UnexpectedText(ref t) if t == "trailing"),
            "{err}"
        );

        let err = pmids(&format!(
            "{head}<DeleteCitation><PMID Version=\"1\">1</PMID>"
        ))
        .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::UnexpectedEof), "{err}");
        assert_eq!(err.path, "PubmedArticleSet/DeleteCitation");

        let err = pmids(&format!("{head}stray</PubmedArticleSet>")).unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::UnexpectedText(ref t) if t == "stray"),
            "{err}"
        );

        let err = pmids(head).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::UnexpectedEof), "{err}");
        assert_eq!(
            pmids(&format!("{head}</PubmedArticleSet>")).unwrap(),
            Vec::<String>::new()
        );
    }
}
