//! The analyzer the tokenizer plan's treatments are built with (T1.1,
//! `docs/tokenizer-plan.md` §4, D2–D6): one Tantivy tokenizer, each change
//! a switch, with two sides.
//!
//! - The **index side** writes a text's own forms and, at the same
//!   positions, the forms another spelling of it would take: `IL6` is
//!   `il6` and, split, `il` and `6`; `IL-6` is `il` and `6` and, joined,
//!   `il6`; `TNF-α` gives `alpha` beside `α`; `µg` gives `ug` beside it.
//! - The **query side** writes one canonical form of each piece — folded,
//!   a Greek letter by its name — so that whatever the query side makes of
//!   a text, the index side made of it too, at the same positions (I2, I3).
//!   A query never asks for every alternative at once.
//!
//! With every switch off ([`Switches::TODAY`]) both sides are Tantivy's
//! `default` analyzer, term for term and position for position: a run of
//! letters and digits a term, lowercased a character at a time, a term of
//! 40 bytes or more dropped where it stood.

use icu_normalizer::ComposingNormalizerBorrowed;
use icu_segmenter::WordSegmenter;
use icu_segmenter::options::WordBreakInvariantOptions;
use serde::{Deserialize, Serialize};
use tantivy::tokenizer::{
    AsciiFoldingFilter, RawTokenizer, TextAnalyzer, Token, TokenStream, Tokenizer,
};

use crate::analysis::{self, Script};

/// How a run of a script written without spaces is split (D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Unspaced {
    /// As today: the whole run one term.
    Whole,
    /// Han, kana and Hangul into overlapping pairs of characters (a lone
    /// character itself); Thai, Lao, Khmer and Burmese into ICU4X's words.
    Bigrams,
    /// All of them into ICU4X's words.
    Words,
}

/// One switch a change (D2–D6); recorded in an index's manifest, and
/// named in the tokenizer it registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Switches {
    /// D3: a piece of this many bytes or more is dropped, as today at 40;
    /// `None`, only the engine's own limit.
    pub cap: Option<usize>,
    /// D2.
    pub unspaced: Unspaced,
    /// D4: NFKC, and diacritics folded.
    pub fold: bool,
    /// D5: a Greek letter alone or joined to other letters or digits
    /// named (`α` is `alpha`); mu or the micro sign in a unit written `u`
    /// (`µg` is `ug`). A Greek word stays as it is.
    pub greek: bool,
    /// D6: a run split where letters meet digits or the script changes,
    /// and two runs joined across one hyphen.
    pub joins: bool,
}

impl Switches {
    /// Tantivy's `default`, as the index is built today.
    pub const TODAY: Self = Self {
        cap: Some(40),
        unspaced: Unspaced::Whole,
        fold: false,
        greek: false,
        joins: false,
    };

    /// The name the index side is registered under; the query side's is
    /// this and [`QUERY_SUFFIX`]. Readable, so a report shows what an index
    /// was built with.
    #[must_use]
    pub fn name(&self) -> String {
        let cap = self
            .cap
            .map_or_else(|| "none".to_owned(), |c| c.to_string());
        let unspaced = match self.unspaced {
            Unspaced::Whole => "whole",
            Unspaced::Bigrams => "bigrams",
            Unspaced::Words => "words",
        };
        format!(
            "ol1-cap{cap}-{unspaced}-fold{}-greek{}-joins{}",
            u8::from(self.fold),
            u8::from(self.greek),
            u8::from(self.joins)
        )
    }
}

/// Appended to [`Switches::name`] for the query side.
pub const QUERY_SUFFIX: &str = ".query";

/// Which side an analyzer writes for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// A text's own forms and every other spelling's, at the same positions.
    Index,
    /// One canonical form of each piece.
    Query,
}

/// The analyzer, as Tantivy registers it.
#[derive(Clone)]
pub struct Analyzer {
    switches: Switches,
    side: Side,
    fold: TextAnalyzer,
    tokens: Vec<Token>,
}

impl std::fmt::Debug for Analyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Analyzer")
            .field("switches", &self.switches)
            .field("side", &self.side)
            .finish_non_exhaustive()
    }
}

impl Analyzer {
    #[allow(missing_docs)]
    #[must_use]
    pub fn new(switches: Switches, side: Side) -> Self {
        Self {
            switches,
            side,
            fold: TextAnalyzer::builder(RawTokenizer::default())
                .filter(AsciiFoldingFilter)
                .build(),
            tokens: Vec::new(),
        }
    }

    /// Registers both sides of `switches` with `index`, under
    /// [`Switches::name`] and that and [`QUERY_SUFFIX`].
    pub fn register(index: &tantivy::Index, switches: Switches) {
        let name = switches.name();
        index
            .tokenizers()
            .register(&name, TextAnalyzer::from(Self::new(switches, Side::Index)));
        index.tokenizers().register(
            &format!("{name}{QUERY_SUFFIX}"),
            TextAnalyzer::from(Self::new(switches, Side::Query)),
        );
    }

    /// The tokens of `text`, as the tokenizer streams them.
    pub fn tokens(&mut self, text: &str) -> Vec<Token> {
        let mut out = Vec::new();
        self.analyze(text, &mut out);
        out
    }

    fn analyze(&mut self, text: &str, out: &mut Vec<Token>) {
        out.clear();
        // With folding, the whole text in NFKC before anything is split, so
        // runs and scripts are read off the characters the terms are made
        // of (a half-width Hangul letter is Hangul once normalized). The
        // tokens' offsets are then the normalized text's; the index keeps
        // positions, not offsets.
        let normalized;
        let text = if self.switches.fold {
            normalized = ComposingNormalizerBorrowed::new_nfkc().normalize(text);
            normalized.as_ref()
        } else {
            text
        };
        let runs = analysis::runs(text);
        let mut position = 0;
        for (i, &(at, run)) in runs.iter().enumerate() {
            let taken = if self.unspaced(run) {
                self.unspaced_run(at, run, position, out)
            } else {
                let taken = self.word(at, run, position, out);
                if self.switches.joins && self.side == Side::Index && taken > 0 {
                    self.hyphen_join(text, &runs, i, position, out);
                }
                taken
            };
            position += taken;
        }
    }

    /// Whether a run is split as a script written without spaces.
    fn unspaced(&self, run: &str) -> bool {
        self.switches.unspaced != Unspaced::Whole
            && run.chars().any(|c| split_script(analysis::script(c)))
    }

    /// A run as one word: its main form, and on the index side its other
    /// spellings; the positions it takes (its parts', when it is split).
    fn word(&mut self, at: usize, run: &str, position: usize, out: &mut Vec<Token>) -> usize {
        let parts = if self.switches.joins && analysis::is_split(run) {
            analysis::parts(run)
        } else {
            vec![run]
        };
        if let Some(cap) = self.switches.cap
            && run.len() >= cap
        {
            // Dropped where it stood, as Tantivy's own cap drops it.
            return parts.len();
        }
        let main = self.main(run);
        self.push(out, at, at + run.len(), position, main);
        if parts.len() > 1 && self.side == Side::Index {
            let mut from = at;
            for (k, part) in parts.iter().enumerate() {
                let form = self.main(part);
                self.push(out, from, from + part.len(), position + k, form);
                from += part.len();
            }
        }
        parts.len()
    }

    /// Two runs joined across one hyphen, written at the first's position:
    /// on the index side only, since the query side's pieces are the two
    /// runs themselves.
    fn hyphen_join(
        &mut self,
        text: &str,
        runs: &[(usize, &str)],
        i: usize,
        position: usize,
        out: &mut Vec<Token>,
    ) {
        const HYPHENS: [char; 4] = ['-', '\u{2010}', '\u{2011}', '\u{2013}'];
        let (Some(&(a_at, a)), Some(&(b_at, b))) = (runs.get(i), runs.get(i + 1)) else {
            return;
        };
        let mut gap = text[a_at + a.len()..b_at].chars();
        let (Some(c), None) = (gap.next(), gap.next()) else {
            return;
        };
        if !HYPHENS.contains(&c) || self.unspaced(b) {
            return;
        }
        let joined = format!("{a}{b}");
        if self
            .switches
            .cap
            .is_some_and(|cap| a.len() >= cap || b.len() >= cap || joined.len() >= cap)
        {
            return;
        }
        let main = self.main(&joined);
        self.push(out, a_at, b_at + b.len(), position, main);
    }

    /// A run of a script written without spaces: its pieces of other
    /// scripts as words, and its unspaced pieces as pairs or as words.
    fn unspaced_run(
        &mut self,
        at: usize,
        run: &str,
        position: usize,
        out: &mut Vec<Token>,
    ) -> usize {
        let mut taken = 0;
        let mut from = at;
        for part in analysis::parts(run) {
            let script = part.chars().next().map_or(Script::Common, analysis::script);
            let pieces: Vec<(usize, &str)> = if !split_script(script) {
                vec![(0, part)]
            } else if self.switches.unspaced == Unspaced::Bigrams && bigram_script(script) {
                bigrams(part)
            } else {
                words(part)
            };
            for (offset, piece) in pieces {
                if self.switches.cap.is_some_and(|cap| piece.len() >= cap) {
                    taken += 1;
                    continue;
                }
                let main = self.main(piece);
                let start = from + offset;
                self.push(out, start, start + piece.len(), position + taken, main);
                taken += 1;
            }
            from += part.len();
        }
        taken
    }

    /// A piece's form as the index writes it and the query asks for it:
    /// lowercased a character at a time, as Tantivy's `LowerCaser`; then,
    /// with folding (the text already in NFKC), diacritics folded.
    fn main(&mut self, piece: &str) -> String {
        let lower: String = piece.chars().flat_map(char::to_lowercase).collect();
        if !self.switches.fold {
            return lower;
        }
        let mut stream = self.fold.token_stream(&lower);
        let mut folded = String::new();
        while stream.advance() {
            folded.push_str(&stream.token().text);
        }
        // Folding can give a capital (`ꟽ`, which has no lowercase, is `M`):
        // lowercased again, so a form is lowercase whatever it was folded to.
        folded.chars().flat_map(char::to_lowercase).collect()
    }

    /// Writes a form, and its Greek spelling where there is one: on the
    /// index side beside it, on the query side in its place.
    fn push(&self, out: &mut Vec<Token>, from: usize, to: usize, position: usize, form: String) {
        let greek = if self.switches.greek {
            greek_spelling(&form)
        } else {
            None
        };
        let mut write = |text: String| {
            if !out
                .iter()
                .rev()
                .take_while(|t| t.position == position)
                .any(|t| t.text == text)
            {
                out.push(Token {
                    offset_from: from,
                    offset_to: to,
                    position,
                    text,
                    position_length: 1,
                });
            }
        };
        match (self.side, greek) {
            (Side::Query, Some(named)) => write(named),
            (Side::Index, Some(named)) => {
                write(form);
                write(named);
            }
            (_, None) => write(form),
        }
    }
}

/// A script whose runs are split when [`Unspaced`] says so: those written
/// without spaces, and Hangul, whose particles stay on their words.
fn split_script(script: Script) -> bool {
    script.unspaced() || script == Script::Hangul
}

/// The scripts split into pairs of characters.
fn bigram_script(script: Script) -> bool {
    matches!(
        script,
        Script::Han | Script::Hiragana | Script::Katakana | Script::Hangul
    )
}

/// Overlapping pairs of characters, each with its byte offset in `part`; a
/// lone character itself.
fn bigrams(part: &str) -> Vec<(usize, &str)> {
    let starts: Vec<usize> = part.char_indices().map(|(i, _)| i).collect();
    if starts.len() < 2 {
        return vec![(0, part)];
    }
    starts
        .windows(2)
        .enumerate()
        .map(|(k, w)| {
            let end = starts.get(k + 2).copied().unwrap_or(part.len());
            (w[0], &part[w[0]..end])
        })
        .collect()
}

/// ICU4X's words by its dictionary model, each with its byte offset in
/// `part`; pieces without a letter or digit left out. Not its LSTM model,
/// which reads a word by its neighbours: `เบาหวาน` (diabetes) is one word
/// in a title and two typed alone, so a query would miss the title (I2).
fn words(part: &str) -> Vec<(usize, &str)> {
    let breaks: Vec<usize> = WordSegmenter::new_dictionary(WordBreakInvariantOptions::default())
        .segment_str(part)
        .collect();
    breaks
        .windows(2)
        .map(|w| (w[0], &part[w[0]..w[1]]))
        .filter(|(_, piece)| piece.chars().any(char::is_alphanumeric))
        .collect()
}

/// A form's Greek spelling (D5), when it has one: a Greek letter alone or
/// joined to other letters or digits by its name; mu or the micro sign in
/// a unit, where it is the only Greek letter, as `u`. A Greek word — two
/// or more Greek letters and nothing else — has none.
fn greek_spelling(form: &str) -> Option<String> {
    let greek = form
        .chars()
        .filter(|c| analysis::greek_name(*c).is_some())
        .count();
    if greek == 0 {
        return None;
    }
    let other = form.chars().any(|c| analysis::greek_name(c).is_none());
    if greek > 1 && !other {
        return None;
    }
    let mu_only = form
        .chars()
        .filter(|c| analysis::greek_name(*c).is_some())
        .all(|c| matches!(c, 'µ' | 'μ' | 'Μ'));
    let spelled: String = if mu_only && other {
        form.chars()
            .map(|c| match c {
                'µ' | 'μ' | 'Μ' => 'u',
                c => c,
            })
            .collect()
    } else {
        let mut out = String::with_capacity(form.len() + 8 * greek);
        for c in form.chars() {
            match analysis::greek_name(c) {
                Some(name) => out.push_str(name),
                None => out.push(c),
            }
        }
        out
    };
    (spelled != form).then_some(spelled)
}

/// The tokens [`Analyzer`] streams: computed whole, then handed out.
#[derive(Debug)]
pub struct Stream<'a> {
    tokens: &'a mut Vec<Token>,
    next: usize,
}

impl TokenStream for Stream<'_> {
    fn advance(&mut self) -> bool {
        if self.next < self.tokens.len() {
            self.next += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.next - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.next - 1]
    }
}

impl Tokenizer for Analyzer {
    type TokenStream<'a> = Stream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Stream<'a> {
        let mut tokens = std::mem::take(&mut self.tokens);
        self.analyze(text, &mut tokens);
        self.tokens = tokens;
        Stream {
            tokens: &mut self.tokens,
            next: 0,
        }
    }
}

/// `text`'s tokens through `analyzer`, as `(position, term)`.
#[cfg(test)]
fn terms(analyzer: &mut TextAnalyzer, text: &str) -> Vec<(usize, String)> {
    let mut stream = analyzer.token_stream(text);
    let mut out = Vec::new();
    while stream.advance() {
        out.push((stream.token().position, stream.token().text.clone()));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use proptest::prelude::*;
    use tantivy::tokenizer::{LowerCaser, RemoveLongFilter, SimpleTokenizer};

    use super::*;

    const ALL: Switches = Switches {
        cap: None,
        unspaced: Unspaced::Bigrams,
        fold: true,
        greek: true,
        joins: true,
    };

    fn side(switches: Switches, side: Side) -> TextAnalyzer {
        TextAnalyzer::from(Analyzer::new(switches, side))
    }

    fn index_terms(switches: Switches, text: &str) -> Vec<(usize, String)> {
        terms(&mut side(switches, Side::Index), text)
    }

    fn query_terms(switches: Switches, text: &str) -> Vec<(usize, String)> {
        terms(&mut side(switches, Side::Query), text)
    }

    fn words_of(terms: &[(usize, String)]) -> Vec<&str> {
        terms.iter().map(|(_, t)| t.as_str()).collect()
    }

    #[test]
    fn scripts_written_without_spaces_are_split() {
        let chinese = index_terms(ALL, "肺癌基因治疗");
        assert_eq!(
            words_of(&chinese),
            vec!["肺癌", "癌基", "基因", "因治", "治疗"]
        );
        let thai = index_terms(ALL, "การศึกษาความชุกของโรคเบาหวาน");
        assert!(thai.len() > 2, "{thai:?}");
        assert!(thai.iter().all(|(_, t)| t.chars().count() < 12), "{thai:?}");
        let mixed = index_terms(ALL, "DNA修复");
        assert_eq!(words_of(&mixed), vec!["dna", "修复"]);
        let words = index_terms(
            Switches {
                unspaced: Unspaced::Words,
                ..ALL
            },
            "新型冠状病毒肺炎患者的临床特征分析",
        );
        assert!(words.len() > 3, "{words:?}");
        // As today, the whole run one term, and over the cap, none.
        assert!(index_terms(Switches::TODAY, "新型冠状病毒肺炎患者的临床特征分析").is_empty());
    }

    #[test]
    fn spellings_meet() {
        assert_eq!(
            index_terms(ALL, "IL6"),
            vec![(0, "il6".into()), (0, "il".into()), (1, "6".into())]
        );
        assert_eq!(
            index_terms(ALL, "IL-6"),
            vec![(0, "il".into()), (0, "il6".into()), (1, "6".into())]
        );
        assert!(words_of(&index_terms(ALL, "TNF-α")).contains(&"alpha"));
        assert_eq!(words_of(&query_terms(ALL, "TNF-α")), vec!["tnf", "alpha"]);
        assert!(words_of(&index_terms(ALL, "5 µg")).contains(&"ug"));
        // ASCII folding folds Latin; a Greek word keeps its tonos.
        assert_eq!(words_of(&query_terms(ALL, "λόγος")), vec!["λόγος"]);
        assert_eq!(
            words_of(&query_terms(ALL, "Étude ＤＮＡ")),
            vec!["etude", "dna"]
        );
        assert_eq!(words_of(&query_terms(ALL, "I²")), vec!["i2"]);
    }

    /// A word ICU4X finds in a title, typed alone, is split as it was in
    /// the title (I2) — which ICU4X's LSTM model fails: `เบาหวาน`
    /// (diabetes), one word in the first sentence, is two alone. Sentences
    /// written for this test in each script ICU4X splits, not the
    /// corpus's titles; the corpus's are T2's census (D11).
    #[test]
    fn a_word_typed_alone_is_split_as_in_its_title() {
        for sentence in [
            "การศึกษาความชุกของโรคเบาหวานในผู้ป่วยสูงอายุ",
            "ผลของการออกกำลังกายต่อความดันโลหิตสูง",
            "ການສຶກສາພະຍາດເບົາຫວານໃນປະເທດລາວ",
            "ការសិក្សាអំពីជំងឺទឹកនោមផ្អែម",
            "မြန်မာနိုင်ငံရှိဆီးချိုရောဂါလေ့လာမှု",
            "新型冠状病毒肺炎患者的临床特征分析",
            "日本における糖尿病患者の臨床的特徴",
        ] {
            let found = words(sentence);
            assert!(found.len() > 3, "{found:?}");
            for (_, word) in found {
                let alone: Vec<&str> = words(word).into_iter().map(|(_, w)| w).collect();
                assert_eq!(alone, vec![word], "in {sentence}");
            }
        }
    }

    #[test]
    fn the_cap_drops_where_it_stood() {
        let today = index_terms(
            Switches::TODAY,
            "a pneumonoultramicroscopicsilicovolcanoconiosis b",
        );
        assert_eq!(today, vec![(0, "a".into()), (2, "b".into())]);
        let none = index_terms(ALL, "a pneumonoultramicroscopicsilicovolcanoconiosis b");
        assert_eq!(none.len(), 3);
    }

    #[test]
    fn names_say_what_was_built() {
        assert_eq!(
            Switches::TODAY.name(),
            "ol1-cap40-whole-fold0-greek0-joins0"
        );
        assert_eq!(ALL.name(), "ol1-capnone-bigrams-fold1-greek1-joins1");
    }

    /// Text from every script T0 counted, punctuation, hyphens and units
    /// among them.
    fn text() -> impl Strategy<Value = String> {
        prop_oneof![
            any::<String>(),
            "[a-zA-Zé0-9α-ωΣµ肺癌基因がんカタ한국กาຢຸ ＤＮＡﬁ² .,;:()\\-‐]{0,80}",
        ]
    }

    fn switches() -> impl Strategy<Value = Switches> {
        (
            prop::option::of(10usize..60),
            prop_oneof![
                Just(Unspaced::Whole),
                Just(Unspaced::Bigrams),
                Just(Unspaced::Words)
            ],
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(|(cap, unspaced, fold, greek, joins)| Switches {
                cap,
                unspaced,
                fold,
                greek,
                joins,
            })
    }

    proptest! {
        /// Every switch off is Tantivy's `default`, term for term and
        /// position for position.
        #[test]
        fn today_is_tantivys_default(text in text()) {
            let mut tantivy = TextAnalyzer::builder(SimpleTokenizer::default())
                .filter(RemoveLongFilter::limit(40))
                .filter(LowerCaser)
                .build();
            prop_assert_eq!(index_terms(Switches::TODAY, &text), terms(&mut tantivy, &text));
            prop_assert_eq!(query_terms(Switches::TODAY, &text), terms(&mut tantivy, &text));
        }

        /// Whatever the query side makes of a text, the index side made of
        /// it too, at the same positions: a text finds itself (I2).
        #[test]
        fn a_text_finds_itself(switches in switches(), text in text()) {
            let mut indexed: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
            for (position, term) in index_terms(switches, &text) {
                indexed.entry(position).or_default().insert(term);
            }
            for (position, term) in query_terms(switches, &text) {
                prop_assert!(
                    indexed.get(&position).is_some_and(|t| t.contains(&term)),
                    "{:?} at {} not indexed in {:?}", term, position, indexed
                );
            }
        }

        /// A term the query side wrote, analyzed again, is itself:
        /// normalizing twice changes nothing. With the cap gone, as D3
        /// decided: a cap measures the text as written, as Tantivy's does,
        /// and a Greek letter spelled out can carry a form past it.
        #[test]
        fn normalizing_twice_changes_nothing(switches in switches(), text in text()) {
            let switches = Switches { cap: None, ..switches };
            for (_, term) in query_terms(switches, &text) {
                let again = query_terms(switches, &term);
                prop_assert!(
                    again.iter().any(|(_, t)| *t == term),
                    "{:?} became {:?}", term, again
                );
            }
        }

        /// With the cap gone and unspaced scripts split, a text with a
        /// letter or digit always yields a term (I1).
        #[test]
        fn text_with_letters_is_findable(
            switches in switches(),
            text in text(),
        ) {
            let switches = Switches { cap: None, unspaced: Unspaced::Bigrams, ..switches };
            if text.chars().any(char::is_alphanumeric) {
                prop_assert!(!index_terms(switches, &text).is_empty());
                prop_assert!(!query_terms(switches, &text).is_empty());
            }
        }
    }
}
