//! The query parser (serving plan §7, 2026-09-25): the text a person
//! typed into the search box, read into the query tree that every search
//! engine behind the serving layer is asked from. Tantivy's own parser
//! read the box until 2026-09-25, and its language was the box's: `:`,
//! quotes, brackets, `^`, `~`, `*`, `+`, `-` and `AND`/`OR`/`NOT` were
//! commands, so `5'-UTR` and `(2R,3S)-dihydroxy` were refused as syntax
//! errors, `id:[1 TO 100]` walked the internal numbering, and twenty
//! nested parentheses took a second to read, each one more doubling it.
//!
//! This parser reads the text once, left to right, and never refuses
//! what it reads; the one refusal is size ([`MAX_BYTES`], [`MAX_WORDS`]).
//!
//! **The language, first pass: plain text.** Every character is text. A
//! word is a run of characters between whitespace, kept exactly as typed.
//! What a word matches is the engine's to say, splitting it as the index
//! split the documents: Tantivy's translation (`search`) makes `IL-6` the
//! two pieces `il` and `6`, next to each other. The language grows —
//! phrases, exclusions, fields, `AND`/`OR`/`NOT`, parentheses — in a
//! later pass (serving plan §11); the tree grows with it, and each
//! engine's translation with the tree.
//!
//! **Positions.** Every word carries its [`Span`] in the text as typed,
//! which is what a client-side highlighter mirroring this parser needs,
//! and what a refusal from a richer language will point at.

use std::fmt;

/// Words a query may hold, as typed: runs of characters between
/// whitespace, before an engine splits them further. The user's number
/// (2026-09-25), counted in words because a person reading the refusal
/// counts words; the engine's own count is bounded by the search's time
/// limit, not here.
pub const MAX_WORDS: usize = 1024;

/// Bytes a query may hold, so that one enormous word with no whitespace
/// in it cannot slip past [`MAX_WORDS`]; checked first, before the text
/// is read at all.
pub const MAX_BYTES: usize = 64 * 1024;

/// Where a piece of a query sits in the text as typed: byte offsets into
/// its UTF-8, the start included and the end not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    #[allow(missing_docs)]
    pub start: usize,
    #[allow(missing_docs)]
    pub end: usize,
}

/// One word, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    /// Never empty, never holding whitespace.
    pub text: String,
    #[allow(missing_docs)]
    pub span: Span,
}

/// A query, read: the text as typed and its words in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    text: String,
    words: Vec<Word>,
}

impl Query {
    /// The text as typed.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The words, in the order they were typed.
    #[must_use]
    pub fn words(&self) -> &[Word] {
        &self.words
    }

    /// Whether there is nothing to look for: no words, which an engine
    /// answers with no hits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

/// A query over one of the caps, with how far over it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooLong {
    /// Over [`MAX_BYTES`].
    Bytes {
        #[allow(missing_docs)]
        bytes: usize,
    },
    /// Over [`MAX_WORDS`].
    Words {
        #[allow(missing_docs)]
        words: usize,
    },
}

impl fmt::Display for TooLong {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bytes { bytes } => {
                write!(f, "the query is {bytes} bytes; the limit is {MAX_BYTES}")
            }
            Self::Words { words } => {
                write!(f, "the query is {words} words; the limit is {MAX_WORDS}")
            }
        }
    }
}

impl std::error::Error for TooLong {}

/// Reads `text`. Any text within the caps is a query; whitespace alone,
/// or nothing, is the query with no words.
///
/// # Errors
///
/// Text over [`MAX_BYTES`], or with more than [`MAX_WORDS`] words.
pub fn parse(text: &str) -> Result<Query, TooLong> {
    if text.len() > MAX_BYTES {
        return Err(TooLong::Bytes { bytes: text.len() });
    }
    let mut words = Vec::new();
    let mut start = None;
    for (at, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(from)) => {
                words.push(word(text, from, at));
                start = None;
            }
            (false, None) => start = Some(at),
            _ => {}
        }
    }
    if let Some(from) = start {
        words.push(word(text, from, text.len()));
    }
    if words.len() > MAX_WORDS {
        return Err(TooLong::Words { words: words.len() });
    }
    Ok(Query {
        text: text.to_owned(),
        words,
    })
}

fn word(text: &str, start: usize, end: usize) -> Word {
    Word {
        text: text[start..end].to_owned(),
        span: Span { start, end },
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use proptest::prelude::*;

    use super::{MAX_BYTES, MAX_WORDS, TooLong, parse};

    /// Characters a query is made of, weighted towards the ones that
    /// were trouble: every whitespace the parser splits on, Tantivy's
    /// syntax, and anything at all.
    fn character() -> impl Strategy<Value = char> {
        prop_oneof![
            4 => any::<char>(),
            2 => prop::sample::select(vec![
                ' ', '\t', '\n', '\r', '\u{a0}', '\u{2003}', '\u{3000}', '\u{2028}',
            ]),
            2 => prop::sample::select(vec![
                '(', ')', '[', ']', '{', '}', '"', '\'', ':', '^', '~', '*', '+', '-', '\\',
                '/', '!', '?',
            ]),
            2 => prop::char::range('a', 'z'),
        ]
    }

    fn text() -> impl Strategy<Value = String> {
        prop::collection::vec(character(), 0..300).prop_map(|cs| cs.into_iter().collect())
    }

    proptest! {
        /// Any text within the caps is read, and the words are the text:
        /// each word's span is exactly that word, the spans are in order
        /// and apart, only whitespace lies outside them, and no word is
        /// empty or holds whitespace. Nothing lost, nothing invented.
        #[test]
        fn any_text_reads_into_its_own_words(text in text()) {
            let query = parse(&text).unwrap();
            prop_assert_eq!(query.text(), text.as_str());
            let mut at = 0;
            for word in query.words() {
                prop_assert!(word.span.start >= at, "{:?} overlaps what came before", word);
                prop_assert!(text[at..word.span.start].chars().all(char::is_whitespace));
                prop_assert_eq!(&text[word.span.start..word.span.end], word.text.as_str());
                prop_assert!(!word.text.is_empty());
                prop_assert!(!word.text.chars().any(char::is_whitespace));
                at = word.span.end;
            }
            prop_assert!(text[at..].chars().all(char::is_whitespace));
            let joined: String = query.words().iter().map(|w| w.text.as_str()).collect();
            let bare: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            prop_assert_eq!(joined, bare);
            prop_assert_eq!(query.is_empty(), query.words().is_empty());
        }

        /// Words are counted as typed, whatever separates them: up to the
        /// cap the query reads with exactly that many words, and past it
        /// the refusal says how many there were.
        #[test]
        fn the_word_cap_counts_words_as_typed(
            words in prop::collection::vec("[^\\s]{1,6}", (MAX_WORDS - 3)..=(MAX_WORDS + 3)),
            gaps in prop::collection::vec(prop::sample::select(vec![" ", "\t", "  \n ", "\u{3000}"]), MAX_WORDS + 4),
        ) {
            let mut text = String::new();
            for (word, gap) in words.iter().zip(&gaps) {
                text.push_str(word);
                text.push_str(gap);
            }
            prop_assume!(text.len() <= MAX_BYTES);
            match parse(&text) {
                Ok(query) => {
                    prop_assert!(words.len() <= MAX_WORDS);
                    prop_assert_eq!(query.words().len(), words.len());
                }
                Err(e) => prop_assert_eq!(e, TooLong::Words { words: words.len() }),
            }
        }
    }

    #[test]
    fn nothing_and_whitespace_are_the_empty_query() {
        for text in ["", " ", "\t\n", "\u{3000}\u{a0}"] {
            let query = parse(text).unwrap();
            assert!(query.is_empty(), "{text:?}");
        }
    }

    #[test]
    fn the_byte_cap_is_exact_and_checked_first() {
        let at_cap = "a".repeat(MAX_BYTES);
        assert_eq!(parse(&at_cap).unwrap().words().len(), 1);
        let over = "a".repeat(MAX_BYTES + 1);
        assert_eq!(
            parse(&over),
            Err(TooLong::Bytes {
                bytes: MAX_BYTES + 1
            })
        );
        // Over both caps: the bytes are what is said, and the words are
        // never counted.
        let both = "a ".repeat(MAX_BYTES);
        assert_eq!(
            parse(&both),
            Err(TooLong::Bytes {
                bytes: 2 * MAX_BYTES
            })
        );
    }

    #[test]
    fn the_refusals_name_the_caps() {
        assert_eq!(
            TooLong::Words { words: 1300 }.to_string(),
            "the query is 1300 words; the limit is 1024"
        );
        assert_eq!(
            TooLong::Bytes { bytes: 70_000 }.to_string(),
            "the query is 70000 bytes; the limit is 65536"
        );
    }

    /// What took Tantivy's parser a second at twenty deep: thirty
    /// thousand, as fast as any other sixty kilobytes.
    #[test]
    fn nesting_is_only_text() {
        let deep = format!("{}aspirin{}", "(".repeat(30_000), ")".repeat(30_000));
        let started = Instant::now();
        let query = parse(&deep).unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(query.words().len(), 1);
    }
}
