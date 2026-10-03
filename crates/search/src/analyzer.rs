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
//! A run of letters and digits is split by its **stretches**: a stretch
//! of a script written without spaces into pairs of characters or
//! ICU4X's words, as [`Unspaced`] says, and any other stretch as a word —
//! so `IL-6基因` is the word `IL-6`, with its joined form, and the pair
//! `基因`, and each switch changes one thing only.
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

/// How a stretch of a script written without spaces is split (D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Unspaced {
    /// As today: not at all — the whole run one term.
    Whole,
    /// Han, kana and Hangul into overlapping pairs of characters, across
    /// the three as a word of Han and kana is written, and in the index
    /// each character beside its pairs, so that any piece of a stretch
    /// finds it, one character or more; Thai, Lao, Khmer and Burmese into
    /// ICU4X's words.
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

    /// The switches a name spells, so a command names them as a report
    /// prints them; `None` for anything [`Switches::name`] does not write,
    /// letter for letter.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        let mut parts = name.split('-');
        if parts.next()? != "ol1" {
            return None;
        }
        let cap = match parts.next()?.strip_prefix("cap")? {
            "none" => None,
            bytes => Some(bytes.parse().ok()?),
        };
        let unspaced = match parts.next()? {
            "whole" => Unspaced::Whole,
            "bigrams" => Unspaced::Bigrams,
            "words" => Unspaced::Words,
            _ => return None,
        };
        let mut flag = |named: &str| match parts.next()?.strip_prefix(named)? {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        };
        let switches = Self {
            cap,
            unspaced,
            fold: flag("fold")?,
            greek: flag("greek")?,
            joins: flag("joins")?,
        };
        (switches.name() == name).then_some(switches)
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
        let mut position = 0;
        // The word the run before ended with, which this run's first word
        // may join across a hyphen: where it starts, itself, its position.
        let mut ended: Option<(usize, &str, usize)> = None;
        for (at, run) in analysis::runs(text) {
            let mut last = None;
            for (k, (offset, stretch, split)) in self.stretches(run).into_iter().enumerate() {
                let start = at + offset;
                let taken = match split {
                    Split::Word => {
                        if k == 0
                            && let Some(before) = ended
                        {
                            self.hyphen_join(text, before, (start, stretch), out);
                        }
                        last = Some((start, stretch, position));
                        self.word(start, stretch, position, out)
                    }
                    Split::Pairs => {
                        last = None;
                        self.pairs(start, stretch, position, out)
                    }
                    Split::Words => {
                        last = None;
                        self.pieces(start, words(stretch), position, out)
                    }
                };
                position += taken;
            }
            ended = last;
        }
        in_order(out);
    }

    /// How a character's stretch of a run is split: scripts written
    /// without spaces as [`Unspaced`] says, anything else — digits among
    /// them — as a word.
    fn split(&self, c: char) -> Split {
        let script = analysis::script(c);
        if self.switches.unspaced == Unspaced::Whole || c.is_numeric() || !split_script(script) {
            Split::Word
        } else if self.switches.unspaced == Unspaced::Bigrams && bigram_script(script) {
            Split::Pairs
        } else {
            Split::Words
        }
    }

    /// A run's stretches — the longest pieces of it split one way — each
    /// with its byte offset in the run. A word glued to a script written
    /// without spaces (`IL6基因`) is a stretch of its own, and a word like
    /// any other; Han beside kana (`新しい`) is one stretch, paired or
    /// divided into words across the two.
    fn stretches<'a>(&self, run: &'a str) -> Vec<(usize, &'a str, Split)> {
        let mut out = Vec::new();
        let mut from = 0;
        let mut last = None;
        for (at, c) in run.char_indices() {
            let split = self.split(c);
            if let Some(before) = last
                && before != split
            {
                out.push((from, &run[from..at], before));
                from = at;
            }
            last = Some(split);
        }
        if let Some(split) = last {
            out.push((from, &run[from..], split));
        }
        out
    }

    /// A stretch as one word: its main form, and on the index side its
    /// other spellings; the positions it takes (its parts', when it is
    /// split).
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

    /// Two words joined across one hyphen — the word a run ended with and
    /// the word the next began with (`IL-6`, and `IL-6基因`) — written at
    /// the first's position: on the index side only, since the query
    /// side's pieces are the two words themselves.
    fn hyphen_join(
        &mut self,
        text: &str,
        (a_at, a, position): (usize, &str, usize),
        (b_at, b): (usize, &str),
        out: &mut Vec<Token>,
    ) {
        const HYPHENS: [char; 4] = ['-', '\u{2010}', '\u{2011}', '\u{2013}'];
        if !self.switches.joins || self.side != Side::Index {
            return;
        }
        let mut gap = text[a_at + a.len()..b_at].chars();
        let (Some(c), None) = (gap.next(), gap.next()) else {
            return;
        };
        if !HYPHENS.contains(&c) {
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

    /// A stretch split into pairs (D2): every two adjacent characters, at
    /// the first's position, so that any two characters or more of the
    /// stretch, typed, are pairs the index wrote side by side. On the
    /// index side each character too, at its own position — Chinese has
    /// words of one character, `癌`, and a character can stand alone
    /// beside a digit, the `型` of `2型` — and on the query side a
    /// character only when it is the whole stretch, since of anything
    /// longer the pairs say more. A stretch takes a position a character,
    /// on both sides, so that what follows it stands beside its last
    /// character whichever was asked for.
    fn pairs(&mut self, at: usize, stretch: &str, position: usize, out: &mut Vec<Token>) -> usize {
        let starts: Vec<usize> = stretch
            .char_indices()
            .map(|(i, _)| i)
            .chain([stretch.len()])
            .collect();
        let characters = starts.len() - 1;
        for i in 0..characters {
            let from = at + starts[i];
            if self.side == Side::Index || characters == 1 {
                self.piece(from, &stretch[starts[i]..starts[i + 1]], position + i, out);
            }
            if i + 1 < characters {
                self.piece(from, &stretch[starts[i]..starts[i + 2]], position + i, out);
            }
        }
        characters
    }

    /// A stretch of a script written without spaces as the words ICU4X
    /// finds in it, each with its byte offset in the stretch: a word a
    /// position.
    fn pieces(
        &mut self,
        at: usize,
        pieces: Vec<(usize, &str)>,
        position: usize,
        out: &mut Vec<Token>,
    ) -> usize {
        let taken = pieces.len();
        for (k, (offset, piece)) in pieces.into_iter().enumerate() {
            self.piece(at + offset, piece, position + k, out);
        }
        taken
    }

    /// One piece of a stretch at its position; one of the cap's length or
    /// more is dropped where it stood.
    fn piece(&mut self, at: usize, piece: &str, position: usize, out: &mut Vec<Token>) {
        if self.switches.cap.is_some_and(|cap| piece.len() >= cap) {
            return;
        }
        let main = self.main(piece);
        self.push(out, at, at + piece.len(), position, main);
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
            out.push(Token {
                offset_from: from,
                offset_to: to,
                position,
                text,
                position_length: 1,
            });
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

/// How a stretch of a run is split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Split {
    /// As a word: one term, and with joins its parts beside it.
    Word,
    /// Into overlapping pairs of characters.
    Pairs,
    /// Into ICU4X's words.
    Words,
}

/// Puts a text's tokens as the engine takes them: in the order of their
/// positions — a term's positions in a text must never go back, and a
/// hyphen join is written at its first word's position after that word's
/// parts — and a form written twice at one position, once.
fn in_order(tokens: &mut Vec<Token>) {
    tokens.sort_by_key(|token| token.position);
    let (mut group, mut i) = (0, 0);
    while i < tokens.len() {
        if tokens[i].position != tokens[group].position {
            group = i;
        }
        if tokens[group..i].iter().any(|t| t.text == tokens[i].text) {
            tokens.remove(i);
        } else {
            i += 1;
        }
    }
}

/// A script whose stretches are split when [`Unspaced`] says so: those
/// written without spaces, and Hangul, whose particles stay on their words.
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

/// ICU4X's words by its dictionary model, each with its byte offset in
/// `stretch`; pieces without a letter or digit left out. Not its LSTM
/// model, which reads a word by its neighbours: `เบาหวาน` (diabetes) is
/// one word in a title and two typed alone, so a query would miss the
/// title (I2). And every word divided again until it is a word alone —
/// a dictionary's words depend on their neighbours too, if less — so
/// that a word of a text, typed alone, is split as the text was.
fn words(stretch: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    divide(0, stretch, &mut out);
    out
}

/// [`words`]: `piece`, which starts at `at`, as the words it is alone, and
/// each of those as the words it is alone, until one is.
fn divide<'a>(at: usize, piece: &'a str, out: &mut Vec<(usize, &'a str)>) {
    let breaks: Vec<usize> = WordSegmenter::new_dictionary(WordBreakInvariantOptions::default())
        .segment_str(piece)
        .collect();
    let found: Vec<(usize, &str)> = breaks
        .windows(2)
        .map(|w| (w[0], &piece[w[0]..w[1]]))
        .filter(|(_, word)| word.chars().any(char::is_alphanumeric))
        .collect();
    if let [(_, whole)] = found[..]
        && whole.len() == piece.len()
    {
        out.push((at, piece));
        return;
    }
    // Each is shorter than `piece`, so this ends.
    for (offset, word) in found {
        divide(at + offset, word, out);
    }
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
pub(crate) mod tests {
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
        // Asked: the pairs. Written: the pairs, and each character at its
        // own position, a position a character.
        assert_eq!(
            words_of(&query_terms(ALL, "肺癌基因治疗")),
            vec!["肺癌", "癌基", "基因", "因治", "治疗"]
        );
        assert_eq!(
            index_terms(ALL, "肺癌基因"),
            vec![
                (0, "肺".into()),
                (0, "肺癌".into()),
                (1, "癌".into()),
                (1, "癌基".into()),
                (2, "基".into()),
                (2, "基因".into()),
                (3, "因".into()),
            ]
        );
        // One character is the whole stretch: itself, on both sides.
        assert_eq!(query_terms(ALL, "癌"), vec![(0, "癌".into())]);
        assert_eq!(index_terms(ALL, "癌"), vec![(0, "癌".into())]);
        let thai = index_terms(ALL, "การศึกษาความชุกของโรคเบาหวาน");
        assert!(thai.len() > 2, "{thai:?}");
        assert!(thai.iter().all(|(_, t)| t.chars().count() < 12), "{thai:?}");
        assert_eq!(words_of(&query_terms(ALL, "DNA修复")), vec!["dna", "修复"]);
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

    /// Whether what the query side makes of `query` stands in what the
    /// index side made of `text`: every piece at its place, counted from
    /// one start — as a phrase is matched.
    fn finds(switches: Switches, query: &str, text: &str) -> bool {
        let asked = query_terms(switches, query);
        let Some(&(first, _)) = asked.first() else {
            return false;
        };
        let indexed: BTreeSet<(usize, String)> = index_terms(switches, text).into_iter().collect();
        let last = indexed.iter().map(|(p, _)| *p).max().unwrap_or(0);
        (0..=last).any(|start| {
            asked
                .iter()
                .all(|(p, term)| indexed.contains(&(start + p - first, term.clone())))
        })
    }

    /// A run is split by its stretches: a word glued to a script written
    /// without spaces is still a word, with its joins; pairs run across
    /// Han and kana, as a word of both is written.
    #[test]
    fn a_run_is_split_by_its_stretches() {
        for title in [
            "IL-6基因多态性",
            "IL6基因多态性",
            "血清IL-6水平",
            "血清IL6水平",
        ] {
            for query in ["IL6", "IL-6", "il6", "基因", "水平", "血清"] {
                let there = title.contains(query)
                    || (query.to_lowercase().starts_with("il") && title.contains("IL"));
                assert_eq!(finds(ALL, query, title), there, "{query:?} in {title:?}");
            }
        }
        assert_eq!(
            words_of(&query_terms(ALL, "新しい治療")),
            vec!["新し", "しい", "い治", "治療"]
        );
        assert!(finds(ALL, "新しい", "新しい治療"));
        assert!(finds(ALL, "しい治", "新しい治療"));
        // Each switch its own change: pairs alone leave `IL6` one term.
        let pairs_only = Switches {
            unspaced: Unspaced::Bigrams,
            ..Switches::TODAY
        };
        assert_eq!(
            query_terms(pairs_only, "IL6基因"),
            vec![(0, "il6".into()), (1, "基因".into())]
        );
        // A digit is a word beside the script: `2型糖尿病`, type 2 diabetes.
        assert_eq!(
            words_of(&query_terms(ALL, "2型糖尿病")),
            vec!["2", "型糖", "糖尿", "尿病"]
        );
    }

    /// One character finds its text (I1): a word of one character, `癌`,
    /// cancer; and a character alone beside a digit or a word, whichever
    /// side it stands on — the `型` of `2型`, type 2.
    #[test]
    fn one_character_finds_its_text() {
        for (query, title) in [
            ("癌", "肺癌基因治疗"),
            ("肺", "肺癌基因治疗"),
            ("疗", "肺癌基因治疗"),
            ("2型", "2型糖尿病"),
            ("2型糖尿病", "2型糖尿病患者"),
            ("型2", "糖尿病型2"),
            ("病型2", "糖尿病型2"),
            ("IL6基", "IL-6基因多态性"),
            ("清IL-6", "血清IL6水平"),
            ("の", "日本における糖尿病患者の臨床的特徴"),
            ("국", "한국어"),
        ] {
            assert!(finds(ALL, query, title), "{query:?} in {title:?}");
        }
        for (query, title) in [
            ("胃", "肺癌基因治疗"),
            ("癌肺", "肺癌基因治疗"),
            ("3型", "2型糖尿病"),
            ("型2", "2型糖尿病"),
        ] {
            assert!(!finds(ALL, query, title), "{query:?} in {title:?}");
        }
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

    /// Any switches, a cap among them; the crate's other tests draw it too.
    pub(crate) fn switches() -> impl Strategy<Value = Switches> {
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
        /// A name is read back as the switches it was written from, and
        /// nothing else is a name.
        #[test]
        fn a_name_reads_back(switches in switches(), other in "[a-z0-9-]{0,40}") {
            prop_assert_eq!(Switches::parse(&switches.name()), Some(switches));
            if let Some(read) = Switches::parse(&other) {
                prop_assert_eq!(read.name(), other);
            }
        }

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

        /// The tokens of any text are as the engine takes them: in the
        /// order of their positions, and no form twice at one position.
        #[test]
        fn tokens_are_as_the_engine_takes_them(switches in switches(), text in text()) {
            for terms in [index_terms(switches, &text), query_terms(switches, &text)] {
                prop_assert!(terms.windows(2).all(|w| w[0].0 <= w[1].0), "{:?}", terms);
                let distinct: BTreeSet<&(usize, String)> = terms.iter().collect();
                prop_assert_eq!(distinct.len(), terms.len(), "{:?}", terms);
            }
        }

        /// Joined or hyphenated, written and asked, a word and its number
        /// meet (I3) — alone, and glued to text of a script written
        /// without spaces when those are split.
        #[test]
        fn a_joined_word_meets_its_hyphenated_one(
            switches in switches(),
            word in "[A-Za-z]{1,4}",
            number in "[0-9]{1,3}",
            before in "[肺癌基因がんカタ한국กา]{0,4}",
            after in "[肺癌基因がんカタ한국กา]{0,4}",
        ) {
            let switches = Switches { cap: None, joins: true, ..switches };
            let (before, after) = if switches.unspaced == Unspaced::Whole {
                ("", "")
            } else {
                (before.as_str(), after.as_str())
            };
            let spellings = [format!("{word}{number}"), format!("{word}-{number}")];
            for written in &spellings {
                let text = format!("{before}{written}{after}");
                for asked in &spellings {
                    prop_assert!(
                        finds(switches, asked, &text),
                        "{:?} in {:?} under {}", asked, text, switches.name()
                    );
                }
            }
        }

        /// Any piece of a stretch split into pairs finds it — one
        /// character or more, whatever stands beside the stretch: what
        /// pairs, and the characters written beside them, are for.
        #[test]
        fn a_piece_of_a_paired_stretch_finds_it(
            switches in switches(),
            stretch in "[肺癌基因新日本がんしいカタ한국어]{1,10}",
            from in any::<prop::sample::Index>(),
            to in any::<prop::sample::Index>(),
            before in "[a-z0-9 กา-]{0,4}",
            after in "[a-z0-9 กา-]{0,4}",
        ) {
            let switches = Switches { cap: None, unspaced: Unspaced::Bigrams, ..switches };
            let chars: Vec<char> = stretch.chars().collect();
            let from = from.index(chars.len());
            let to = from + 1 + to.index(chars.len() - from);
            let piece: String = chars[from..to].iter().collect();
            let text = format!("{before}{stretch}{after}");
            prop_assert!(
                finds(switches, &piece, &text),
                "{:?} in {:?} under {}", piece, text, switches.name()
            );
        }

        /// The end of a paired stretch and the word glued to it, typed
        /// together, find the text, on either side of the stretch: a
        /// stretch takes a position a character, so the word stands
        /// beside its last one whether a character or a pair was asked.
        #[test]
        fn the_end_of_a_stretch_and_the_word_beside_it_find_the_text(
            switches in switches(),
            stretch in "[肺癌基因新日本がんしいカタ한국어]{1,6}",
            word in "[a-z]{1,4}|[0-9]{1,3}",
            taken in 1usize..4,
            word_first in any::<bool>(),
        ) {
            let switches = Switches { cap: None, unspaced: Unspaced::Bigrams, ..switches };
            let chars: Vec<char> = stretch.chars().collect();
            let taken = taken.min(chars.len());
            let (text, asked) = if word_first {
                let end: String = chars[..taken].iter().collect();
                (format!("{word}{stretch}"), format!("{word}{end}"))
            } else {
                let end: String = chars[chars.len() - taken..].iter().collect();
                (format!("{stretch}{word}"), format!("{end}{word}"))
            };
            prop_assert!(
                finds(switches, &asked, &text),
                "{:?} in {:?} under {}", asked, text, switches.name()
            );
        }
    }
}
