//! The query log (2026-09-25): one line per search on standard output,
//! apart from the fault log on standard error. The fault log is how the
//! old parser's trouble surfaced — a typo in the search box printed as
//! the server's fault — and it keeps only the machines' trouble now. What
//! people asked, and what became of it, is here: the zero-hit queries are
//! where the tokenizer fails them, the slow ones what a search costs.
//!
//! ```text
//! ol-rht-serve query: answered bytes=24 words=3 hits=120 ms=41 | IL-6 "heart failure"
//! ```
//!
//! The outcome is one of [`Outcome`]'s names. `words` is `-` where the
//! query was not read and `hits` where it was not answered. The query
//! comes last, after ` | `, so nothing in it can be taken for a field:
//! whole up to the parser's byte cap (`bytes` says how much there was
//! past it), and inert ([`inert`]) — the user's condition for writing it
//! whole (2026-09-25): readable, but nothing in it can act on a terminal
//! or hide itself in a copy and paste.

use std::time::Duration;

use query_parser::MAX_BYTES;

/// What became of a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// Hits, perhaps none.
    Answered,
    /// Over one of the parser's caps: refused, and a client's to fix.
    TooLong,
    /// No search thread free within the time limit: the machines'
    /// trouble, written to the fault log too.
    Busy,
    /// Ran past the time limit.
    TimedOut,
    /// The engine failed: the fault log has why.
    Failed,
}

impl Outcome {
    const fn name(self) -> &'static str {
        match self {
            Self::Answered => "answered",
            Self::TooLong => "too-long",
            Self::Busy => "busy",
            Self::TimedOut => "timed-out",
            Self::Failed => "failed",
        }
    }
}

/// Writes one search's line.
pub(crate) fn log(
    outcome: Outcome,
    text: &str,
    words: Option<usize>,
    hits: Option<u64>,
    elapsed: Duration,
) {
    println!("{}", line(outcome, text, words, hits, elapsed));
}

fn line(
    outcome: Outcome,
    text: &str,
    words: Option<usize>,
    hits: Option<u64>,
    elapsed: Duration,
) -> String {
    let dash = |n: Option<String>| n.unwrap_or_else(|| "-".to_owned());
    format!(
        "ol-rht-serve query: {} bytes={} words={} hits={} ms={} | {}",
        outcome.name(),
        text.len(),
        dash(words.map(|n| n.to_string())),
        dash(hits.map(|n| n.to_string())),
        elapsed.as_millis(),
        inert(&text[..text.floor_char_boundary(MAX_BYTES)]),
    )
}

/// `text` with every character that is not plainly visible written as an
/// escape — control characters and terminal escape sequences, line and
/// paragraph separators, right-to-left overrides, zero-width and other
/// invisible characters, every space but the space bar's, and combining
/// marks, so that a decomposed `é` reads `e\u{301}` — and a backslash as
/// `\\`, so that every escape is one. Letters, digits, punctuation and
/// symbols of any script are written as typed.
///
/// Plain text and not JSON, on purpose: a JSON string's escapes are
/// undone by whatever reads it (`jq -r` writes a `\u202e` back out as the
/// override itself), and these are undone by nothing.
pub(crate) fn inert(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            // Visible, and `escape_debug` would escape them for a Rust
            // literal's sake, not a reader's.
            '"' | '\'' => out.push(c),
            _ => out.extend(c.escape_debug()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use proptest::prelude::*;
    use query_parser::MAX_BYTES;

    use super::{Outcome, inert, line};

    /// `inert` undone, which nothing outside these tests does.
    fn undone(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('0') => out.push('\0'),
                Some('u') => {
                    assert_eq!(chars.next(), Some('{'));
                    let hex: String = chars.by_ref().take_while(|c| *c != '}').collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap());
                }
                other => panic!("\\{other:?} is no escape inert writes"),
            }
        }
        out
    }

    /// Characters weighted towards the dangerous ones.
    fn text() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop_oneof![
                3 => any::<char>(),
                2 => prop::sample::select(vec![
                    '\u{1b}', '\u{7}', '\u{7f}', '\u{9b}', '\n', '\r', '\t', '\0', '\u{202e}',
                    '\u{2066}', '\u{200b}', '\u{feff}', '\u{2028}', '\u{a0}', '\u{301}', '\\',
                ]),
                2 => prop::char::range(' ', '~'),
            ],
            0..80,
        )
        .prop_map(|cs| cs.into_iter().collect())
    }

    proptest! {
        /// What `inert` writes is plainly visible, character by
        /// character: nothing a terminal acts on, nothing invisible.
        #[test]
        fn what_it_writes_is_visible(text in text()) {
            for c in inert(&text).chars() {
                prop_assert!(
                    matches!(c, '\\' | '"' | '\'') || c.escape_debug().eq([c]),
                    "{:?} written as is", c
                );
            }
        }

        /// And it is the whole query: undone, it is what was typed.
        #[test]
        fn what_it_writes_is_the_whole_query(text in text()) {
            prop_assert_eq!(undone(&inert(&text)), text);
        }
    }

    #[test]
    fn the_readable_is_written_as_typed() {
        for text in [
            "IL-6 \"heart failure\" -mice",
            "α-synuclein (2R,3S)-dihydroxy 5'-UTR Na+/K+-ATPase",
            "p<0.05 10.1000/xyz doi:10.1000/x 中文 العربية",
        ] {
            assert_eq!(inert(text), text);
        }
    }

    #[test]
    fn the_dangerous_is_written_as_escapes() {
        for (text, written) in [
            ("\u{1b}[2J", "\\u{1b}[2J"),
            ("a\nb", "a\\nb"),
            ("\u{202e}evil", "\\u{202e}evil"),
            ("zero\u{200b}width", "zero\\u{200b}width"),
            ("e\u{301}", "e\\u{301}"),
            ("\u{9b}31m", "\\u{9b}31m"),
            ("C:\\path", "C:\\\\path"),
        ] {
            assert_eq!(inert(text), written, "{text:?}");
        }
    }

    #[test]
    fn a_line_says_what_became_of_a_search() {
        assert_eq!(
            line(
                Outcome::Answered,
                "IL-6 \"heart failure\"",
                Some(3),
                Some(120),
                Duration::from_millis(41)
            ),
            "ol-rht-serve query: answered bytes=20 words=3 hits=120 ms=41 | IL-6 \"heart failure\""
        );
        assert_eq!(
            line(
                Outcome::Busy,
                "a\nb",
                Some(2),
                None,
                Duration::from_secs(10)
            ),
            "ol-rht-serve query: busy bytes=3 words=2 hits=- ms=10000 | a\\nb"
        );
    }

    /// A query over the byte cap is written up to it, and says how much
    /// there was.
    #[test]
    fn a_long_query_is_written_to_the_cap() {
        let text = "é".repeat(MAX_BYTES);
        let written = line(Outcome::TooLong, &text, None, None, Duration::ZERO);
        assert!(written.starts_with(&format!(
            "ol-rht-serve query: too-long bytes={} words=- hits=- ms=0 | ",
            2 * MAX_BYTES
        )));
        let (_, query) = written.split_once(" | ").unwrap();
        assert_eq!(query.len(), MAX_BYTES);
    }
}
