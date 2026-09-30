//! The query tree ([`query_parser`]) as Tantivy's query: the one place
//! the tree meets this engine, so that another engine is another
//! translation of the same tree.
//!
//! A word is split by each searched field's own analyzer, exactly as the
//! field's text was split when it was indexed, and matched in every
//! searched field: one piece is that term; several pieces — `IL-6` is
//! `il` and `6` — are those terms side by side, a phrase; no piece at
//! all — `?`, an emoji — matches nothing and is dropped. Every word in
//! every field is one alternative in one flat list, any one of them a
//! match, and a word typed twice, character for character, counts once.
//! That is what Tantivy's own parser made of plain words until
//! 2026-09-25 — the flat list is its `simplify`, the word counted once
//! its grammar's — so a query of plain words finds what it found and
//! scores as it scored; the property test below holds the two to it.

use std::collections::HashSet;

use query_parser::Query as Asked;
use tantivy::Index;
use tantivy::query::{BooleanQuery, EmptyQuery, Occur, PhraseQuery, Query, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Term};

use crate::error::Error;

/// `asked` as a query over `fields` — the search box's
/// ([`crate::schema::Fields::searched`]) or a citation's
/// ([`crate::schema::Fields::cited`]).
///
/// # Errors
///
/// A searched field with no analyzer: another mapping's index.
pub(crate) fn query(
    index: &Index,
    fields: Vec<Field>,
    asked: &Asked,
) -> Result<Box<dyn Query>, Error> {
    let mut analyzers = fields
        .into_iter()
        .map(|field| Ok((field, index.tokenizer_for_field(field)?)))
        .collect::<Result<Vec<_>, Error>>()?;
    let mut seen = HashSet::new();
    let mut matches: Vec<Box<dyn Query>> = Vec::new();
    for word in asked.words() {
        if !seen.insert(word.text.as_str()) {
            continue;
        }
        for (field, analyzer) in &mut analyzers {
            let mut pieces: Vec<(usize, Term)> = Vec::new();
            analyzer.token_stream(&word.text).process(&mut |token| {
                pieces.push((token.position, Term::from_field_text(*field, &token.text)));
            });
            match pieces.len() {
                0 => {}
                1 => {
                    let (_, term) = pieces.pop().expect("one piece");
                    matches.push(Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs)));
                }
                _ => matches.push(Box::new(PhraseQuery::new_with_offset(pieces))),
            }
        }
    }
    Ok(match matches.len() {
        0 => Box::new(EmptyQuery),
        1 => matches.pop().expect("one match"),
        _ => Box::new(BooleanQuery::new(
            matches.into_iter().map(|q| (Occur::Should, q)).collect(),
        )),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::OnceLock;

    use merge::kind::{Form, Genre};
    use merge::{Document, SourceId, Sources, WorkId};
    use proptest::prelude::*;
    use tantivy::collector::{Count, TopDocs};
    use tantivy::query::QueryParser;
    use tantivy::{DocAddress, Index, IndexWriter, Searcher};

    use crate::schema::{self, Fields};

    /// Words the documents are written from, punctuation and all.
    const VOCABULARY: [&str; 24] = [
        "aspirin",
        "trial",
        "randomized",
        "of",
        "the",
        "heart",
        "failure",
        "IL-6",
        "COVID-19",
        "α-synuclein",
        "Na+/K+-ATPase",
        "10.1000/xyz",
        "5'-UTR",
        "(2R,3S)-dihydroxy",
        "mice",
        "rats",
        "p<0.05",
        "C++",
        "e.g.",
        "Lovelace",
        "journal",
        "tests",
        "β-amyloid",
        "levels",
    ];

    fn document(id: u32, title: &str, abstract_text: &str) -> Document {
        Document {
            id: WorkId::new(id).unwrap(),
            title: Some(title.to_owned()),
            abstract_text: Some(abstract_text.to_owned()),
            authors: vec!["Ada Lovelace".to_owned()],
            venue: Some("Journal of Tests".to_owned()),
            year: Some(2020),
            form: Some(Form::JournalArticle),
            genre: Some(Genre::Article),
            language: None,
            retracted: false,
            corrected: false,
            concern: false,
            access: Some(true),
            held_by: [SourceId::Pubmed].into_iter().collect::<Sources>(),
        }
    }

    /// An index in memory holding `documents`.
    fn index(documents: &[Document]) -> (Index, Fields) {
        let (schema, fields) = schema::provisional();
        let index = Index::create_in_ram(schema);
        let mut writer: IndexWriter = index.writer_with_num_threads(1, 15_000_000).unwrap();
        for d in documents {
            writer
                .add_document(crate::index::document(&fields, d).unwrap())
                .unwrap();
        }
        writer.commit().unwrap();
        (index, fields)
    }

    /// Forty documents from the vocabulary, the same forty every run.
    fn corpus() -> &'static (Index, Fields) {
        static CORPUS: OnceLock<(Index, Fields)> = OnceLock::new();
        CORPUS.get_or_init(|| {
            let mut state: u32 = 7;
            let mut next = move || {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (state >> 16) as usize % VOCABULARY.len()
            };
            let documents: Vec<Document> = (1..=40)
                .map(|id| {
                    let title: Vec<&str> = (0..4).map(|_| VOCABULARY[next()]).collect();
                    let text: Vec<&str> = (0..12).map(|_| VOCABULARY[next()]).collect();
                    document(id, &title.join(" "), &text.join(" "))
                })
                .collect();
            index(&documents)
        })
    }

    /// Every hit and its score, by document.
    fn hits(searcher: &Searcher, query: &dyn tantivy::query::Query) -> BTreeMap<DocAddress, f32> {
        searcher
            .search(query, &TopDocs::with_limit(1000).order_by_score())
            .unwrap()
            .into_iter()
            .map(|(score, address)| (address, score))
            .collect()
    }

    fn ours(index: &Index, fields: &Fields, text: &str) -> Box<dyn tantivy::query::Query> {
        super::query(
            index,
            fields.searched(),
            &query_parser::parse(text).unwrap(),
        )
        .unwrap()
    }

    /// A word Tantivy's own parser read as a plain word: letters, digits
    /// and Greek, with inner hyphens, dots and slashes, never one of its
    /// operators.
    fn plain_word() -> impl Strategy<Value = String> {
        prop_oneof![
            prop::sample::select(VOCABULARY.to_vec())
                .prop_filter("words its parser read as syntax", |w| {
                    !w.contains(['(', '\'', '<', '+'])
                })
                .prop_map(str::to_owned),
            "[a-z0-9α-ω]{1,6}([-./][a-z0-9α-ω]{1,6}){0,2}",
        ]
    }

    proptest! {
        /// Where Tantivy's parser read a query at all, as plain words, the
        /// translation finds the same documents with the same scores.
        #[test]
        fn plain_words_find_what_tantivys_parser_found(
            words in prop::collection::vec(plain_word(), 1..6),
        ) {
            let (index, fields) = corpus();
            let text = words.join(" ");
            let theirs = QueryParser::for_index(index, fields.searched()).parse_query(&text).unwrap();
            let searcher = index.reader().unwrap().searcher();
            let (a, b) = (hits(&searcher, &*theirs), hits(&searcher, &*ours(index, fields, &text)));
            prop_assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
            for (address, score) in &a {
                prop_assert!((score - b[address]).abs() <= 1e-5 * score.abs().max(1.0), "{text:?} {address:?}");
            }
        }

        /// Any word of a document's title that the index split into at
        /// least one piece finds that document — whatever punctuation,
        /// script or syntax the word is made of.
        #[test]
        fn a_word_from_a_title_finds_its_document(
            title in prop::collection::vec("[^\\s]{1,12}", 1..8),
            pick in any::<prop::sample::Index>(),
        ) {
            let text = title.join(" ");
            let (index, fields) = index(&[document(1, &text, "")]);
            let word = pick.get(&title);
            let mut analyzer = index.tokenizer_for_field(fields.title).unwrap();
            let mut pieces = 0;
            analyzer.token_stream(word).process(&mut |_| pieces += 1);
            prop_assume!(pieces > 0);
            let searcher = index.reader().unwrap().searcher();
            let found = searcher.search(&*ours(&index, &fields, word), &Count).unwrap();
            prop_assert_eq!(found, 1, "{:?} in {:?}", word, text);
        }
    }

    /// The inputs of the probe that found the old parser's trouble
    /// (2026-09-25), which refused half of them, took some for commands,
    /// and spun on the nested ones. The list to grow.
    const PROBED: &[&str] = &[
        "",
        " ",
        "\t\n",
        "\"",
        "\"\"",
        "\"unbalanced",
        "aspirin\"",
        "(",
        ")",
        "((",
        "())",
        "[",
        "]",
        "{",
        "}",
        ":",
        "title:",
        ":aspirin",
        "title:aspirin",
        "nosuchfield:aspirin",
        "http://example.com",
        "^",
        "aspirin^",
        "aspirin^2",
        "aspirin^0",
        "aspirin^-1",
        "~",
        "\"aspirin trial\"~",
        "\"aspirin trial\"~4294967295",
        "*",
        "**",
        "a*",
        "\"asp\"*",
        "?",
        "a?b",
        "-aspirin",
        "+",
        "-",
        "+-",
        "--aspirin",
        "NOT aspirin",
        "AND",
        "OR",
        "NOT",
        "aspirin AND",
        "AND aspirin",
        "aspirin AND OR trial",
        "!",
        "\\",
        "\\\"",
        "/",
        "/asp.*/",
        "title:/asp.*/",
        "id:1",
        "id:abc",
        "id:[1 TO 5]",
        "id:>1",
        "id:<=2",
        "title:[a TO z]",
        "title:{* TO *}",
        "title:*",
        "id:*",
        "facet:/year/2020",
        "facet:year",
        "facet:*",
        "IN [aspirin]",
        "title: IN [aspirin trial]",
        "COVID-19",
        "IL-6",
        "p<0.05",
        "5'-UTR",
        "C++",
        "α-synuclein",
        "Na+/K+-ATPase",
        "(2R,3S)-dihydroxy",
        "10.1000/xyz",
        "doi:10.1000/x",
        "pmid:123",
        "e.g.",
        "50%",
        "#hashtag",
        "@",
        "&",
        "a&&b",
        "a||b",
        "🧬 aspirin",
        "\u{0}",
        "\u{FFFF}",
        "e\u{301}",
        "\u{202E}aspirin",
        "(((((((((((((((((((((((((((((((aspirin)))))))))))))))))))))))))))))))",
        "((((((((((((((((((((((((((((((",
    ];

    /// Each of the probe's inputs is now a query, answered.
    #[test]
    fn the_probes_inputs_are_all_answered() {
        let (index, fields) = corpus();
        let searcher = index.reader().unwrap().searcher();
        for text in PROBED {
            let query = ours(index, fields, text);
            searcher
                .search(&*query, &Count)
                .unwrap_or_else(|e| panic!("{text:?}: {e}"));
        }
    }

    /// Text Tantivy's parser refused, or read as a command, is text: each
    /// finds the document it was written in and no other.
    #[test]
    fn syntax_is_text() {
        let (index, fields) = index(&[
            document(1, "5'-UTR and (2R,3S)-dihydroxy", "see doi:10.1000/x"),
            document(2, "aspirin AND headache", "id 1 of http://example.com"),
            document(3, "Something else entirely", "nothing here"),
        ]);
        let searcher = index.reader().unwrap().searcher();
        let found = |text: &str| {
            let mut ids: Vec<u32> = hits(&searcher, &*ours(&index, &fields, text))
                .keys()
                .map(|a| a.doc_id + 1)
                .collect();
            ids.sort_unstable();
            ids
        };
        for (text, expected) in [
            ("5'-UTR", vec![1]),
            ("(2R,3S)-dihydroxy", vec![1]),
            ("doi:10.1000/x", vec![1]),
            ("http://example.com", vec![2]),
            // Any word a match: `and` is in the first document's title.
            ("aspirin AND", vec![1, 2]),
            ("-aspirin", vec![2]),
            ("\"aspirin", vec![2]),
            // `id:1` asked for work 1 by its internal id; now it is the
            // two pieces `id` and `1`, side by side, which only the
            // second document holds.
            ("id:1", vec![2]),
            // Three words, the first of them `id` and `1` side by side.
            ("id:[1 TO 5000]", vec![2]),
            ("*", vec![]),
            ("aspirin^99999", vec![]),
            ("?", vec![]),
            ("", vec![]),
        ] {
            assert_eq!(found(text), expected, "{text:?}");
        }
    }
}
