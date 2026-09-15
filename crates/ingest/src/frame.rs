//! Record framing: cutting a raw file's byte stream into one record's text
//! at a time, so a worker holds one record and never a file (the
//! streaming contract of plan §3.2, and §5's memory bound). The helpers
//! here are the framings sources share; a source with a framing of its
//! own — PubMed's XML — writes it on the same shape.

use std::error::Error;
use std::fmt;
use std::io::{self, BufRead};
use std::ops::ControlFlow;

/// A line buffer's starting size; it grows to the longest line.
const LINE: usize = 16 * 1024;

/// Why reading a file's records stopped early: the stream, or a record.
#[derive(Debug)]
pub enum ReadError<P> {
    /// The stream: I/O, gzip, or bytes that are not UTF-8.
    Io(io::Error),
    /// A record that did not parse, in the source's own terms.
    Parse(P),
}

impl<P> From<io::Error> for ReadError<P> {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl<P: fmt::Display> fmt::Display for ReadError<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Parse(e) => write!(f, "{e}"),
        }
    }
}

impl<P: Error + 'static> Error for ReadError<P> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
        }
    }
}

/// The text of `bytes`.
///
/// # Errors
///
/// `InvalidData` when they are not UTF-8: the files declare it, and
/// anything else is a difference worth stopping for.
pub fn utf8(bytes: &[u8]) -> io::Result<&str> {
    std::str::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// One record per line. Calls `each` with every non-blank line and its
/// number from one (blank lines are counted, not passed), the line
/// borrowed from one buffer reused for the next, without its newline
/// (`\r\n` too). `each` returns the line's parse error, or whether to go
/// on.
///
/// # Errors
///
/// The stream's, or the first parse error `each` returned.
pub fn lines<R, P, F>(mut input: R, mut each: F) -> Result<(), ReadError<P>>
where
    R: BufRead,
    F: FnMut(usize, &str) -> Result<ControlFlow<()>, P>,
{
    let mut buf = Vec::with_capacity(LINE);
    let mut number = 0;
    loop {
        buf.clear();
        if input.read_until(b'\n', &mut buf)? == 0 {
            return Ok(());
        }
        number += 1;
        let line = utf8(&buf)?;
        let line = line.strip_suffix('\n').unwrap_or(line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            continue;
        }
        match each(number, line) {
            Ok(ControlFlow::Continue(())) => {}
            Ok(ControlFlow::Break(())) => return Ok(()),
            Err(e) => return Err(ReadError::Parse(e)),
        }
    }
}

/// Whether the stream, past any leading ASCII whitespace, starts with
/// `prefix`. Consumes nothing, and looks only as far as the reader's
/// buffer, which is plenty for a container's first bytes.
///
/// # Errors
///
/// The stream's.
pub fn starts_with<R: BufRead>(input: &mut R, prefix: &[u8]) -> io::Result<bool> {
    Ok(input.fill_buf()?.trim_ascii_start().starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};
    use std::ops::ControlFlow;

    use super::{ReadError, lines, starts_with, utf8};

    #[test]
    fn lines_are_numbered_with_blanks_counted_and_endings_stripped() {
        let mut seen = Vec::new();
        lines::<_, (), _>(Cursor::new("a\r\n\n  \nb\nc"), |n, line| {
            seen.push((n, line.to_owned()));
            Ok(ControlFlow::Continue(()))
        })
        .unwrap();
        assert_eq!(
            seen,
            [
                (1, "a".to_owned()),
                (4, "b".to_owned()),
                (5, "c".to_owned())
            ]
        );
    }

    #[test]
    fn break_stops_before_the_next_line_and_a_parse_error_is_returned() {
        let mut seen = 0;
        lines::<_, (), _>(Cursor::new("a\nb\nc\n"), |_, _| {
            seen += 1;
            Ok(ControlFlow::Break(()))
        })
        .unwrap();
        assert_eq!(seen, 1);
        let err = lines(Cursor::new("a\nbad\nc\n"), |n, line| {
            if line == "bad" {
                Err(n)
            } else {
                Ok(ControlFlow::Continue(()))
            }
        })
        .unwrap_err();
        assert!(matches!(err, ReadError::Parse(2)));
        assert_eq!(err.to_string(), "2");
    }

    #[test]
    fn bytes_that_are_not_utf8_are_an_io_error() {
        let err = lines::<_, (), _>(Cursor::new(b"ok\n\xff\n".as_slice()), |_, _| {
            Ok(ControlFlow::Continue(()))
        })
        .unwrap_err();
        assert!(matches!(&err, ReadError::Io(e) if e.kind() == std::io::ErrorKind::InvalidData));
        assert!(utf8(b"\xff").is_err());
    }

    #[test]
    fn starts_with_skips_leading_whitespace_and_consumes_nothing() {
        let mut input = BufReader::new(Cursor::new("  \n{\"status\":1}"));
        assert!(starts_with(&mut input, b"{\"status\"").unwrap());
        assert!(!starts_with(&mut input, b"{\"x\"").unwrap());
        let mut rest = String::new();
        std::io::Read::read_to_string(&mut input, &mut rest).unwrap();
        assert_eq!(rest, "  \n{\"status\":1}");
    }
}
