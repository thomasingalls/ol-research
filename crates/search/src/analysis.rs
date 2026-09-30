//! Analysis as the tokenizer plan measures it (`docs/tokenizer-plan.md`):
//! the analyzer the index uses today, and, for T0, each candidate change
//! of §4 as a function over terms, so the census and the miss analysis
//! can ask what a change would do before anything is rebuilt.
//!
//! Scripts and character classes are read by Unicode block: a census
//! counts classes, and a block says which class a character is in without
//! a table of its own. The Greek letters' names are a table, checked
//! against every letter of the basic Greek alphabet.

use std::borrow::Cow;

use tantivy::tokenizer::{
    AsciiFoldingFilter, Language, LowerCaser, RawTokenizer, RemoveLongFilter, SimpleTokenizer,
    Stemmer, TextAnalyzer, TokenStream,
};

/// The default analyzer's length cap, in bytes: a term this long or
/// longer is dropped.
pub const CAP: usize = 40;

/// The analyzers T0 asks with. Each is built once and used for many texts;
/// Tantivy's analyzers are driven through `&mut`.
#[allow(missing_debug_implementations)]
pub struct Analyzers {
    today: TextAnalyzer,
    uncapped: TextAnalyzer,
    fold: TextAnalyzer,
    stem: TextAnalyzer,
}

impl Default for Analyzers {
    fn default() -> Self {
        Self {
            // Tantivy's `default`, as `crates/search/src/schema.rs` indexes.
            today: TextAnalyzer::builder(SimpleTokenizer::default())
                .filter(RemoveLongFilter::limit(CAP))
                .filter(LowerCaser)
                .build(),
            uncapped: TextAnalyzer::builder(SimpleTokenizer::default())
                .filter(LowerCaser)
                .build(),
            fold: TextAnalyzer::builder(RawTokenizer::default())
                .filter(AsciiFoldingFilter)
                .build(),
            stem: TextAnalyzer::builder(RawTokenizer::default())
                .filter(Stemmer::new(Language::English))
                .build(),
        }
    }
}

fn collect(analyzer: &mut TextAnalyzer, text: &str) -> Vec<String> {
    let mut stream = analyzer.token_stream(text);
    let mut out = Vec::new();
    while stream.advance() {
        out.push(stream.token().text.clone());
    }
    out
}

impl Analyzers {
    /// The terms the index makes of `text` today.
    pub fn terms(&mut self, text: &str) -> Vec<String> {
        collect(&mut self.today, text)
    }

    /// The terms without the length cap (D3): every run, lowercased.
    pub fn uncapped(&mut self, text: &str) -> Vec<String> {
        collect(&mut self.uncapped, text)
    }

    /// D4: a term with its diacritics folded (`étude` to `etude`), by
    /// Tantivy's ASCII folding, which also folds full-width Latin.
    pub fn folded(&mut self, term: &str) -> String {
        collect(&mut self.fold, term).concat()
    }

    /// D8: a term as the English stemmer leaves it.
    pub fn stemmed(&mut self, term: &str) -> String {
        collect(&mut self.stem, term).concat()
    }
}

/// The runs of letters and digits in `text`, before any cap or case, with
/// their byte offsets: what the default tokenizer splits text into.
#[must_use]
pub fn runs(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (at, c) in text.char_indices() {
        match (c.is_alphanumeric(), start) {
            (true, None) => start = Some(at),
            (false, Some(from)) => {
                out.push((from, &text[from..at]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        out.push((from, &text[from..]));
    }
    out
}

/// A script, by Unicode block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum Script {
    Latin,
    Greek,
    Cyrillic,
    Arabic,
    Hebrew,
    Devanagari,
    Han,
    Hiragana,
    Katakana,
    Hangul,
    Thai,
    Lao,
    Khmer,
    Myanmar,
    /// Digits, punctuation and every block not named.
    Common,
}

impl Script {
    /// Every script, in order.
    pub const ALL: [Self; 15] = [
        Self::Latin,
        Self::Greek,
        Self::Cyrillic,
        Self::Arabic,
        Self::Hebrew,
        Self::Devanagari,
        Self::Han,
        Self::Hiragana,
        Self::Katakana,
        Self::Hangul,
        Self::Thai,
        Self::Lao,
        Self::Khmer,
        Self::Myanmar,
        Self::Common,
    ];

    /// Written without spaces between words: a run of it is a clause, not
    /// a word (tokenizer plan §3).
    #[must_use]
    pub const fn unspaced(self) -> bool {
        matches!(
            self,
            Self::Han
                | Self::Hiragana
                | Self::Katakana
                | Self::Thai
                | Self::Lao
                | Self::Khmer
                | Self::Myanmar
        )
    }

    /// The name a report prints.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Latin => "Latin",
            Self::Greek => "Greek",
            Self::Cyrillic => "Cyrillic",
            Self::Arabic => "Arabic",
            Self::Hebrew => "Hebrew",
            Self::Devanagari => "Devanagari",
            Self::Han => "Han",
            Self::Hiragana => "Hiragana",
            Self::Katakana => "Katakana",
            Self::Hangul => "Hangul",
            Self::Thai => "Thai",
            Self::Lao => "Lao",
            Self::Khmer => "Khmer",
            Self::Myanmar => "Myanmar",
            Self::Common => "Common",
        }
    }
}

/// The script of a character, by its Unicode block.
#[must_use]
pub fn script(c: char) -> Script {
    match u32::from(c) {
        0x41..=0x5A
        | 0x61..=0x7A
        | 0xC0..=0x24F
        | 0x1E00..=0x1EFF
        | 0xFF21..=0xFF3A
        | 0xFF41..=0xFF5A
            if c.is_alphabetic() =>
        {
            Script::Latin
        }
        0xB5 | 0x370..=0x3FF | 0x1F00..=0x1FFF => Script::Greek,
        0x400..=0x52F => Script::Cyrillic,
        0x600..=0x6FF | 0x750..=0x77F | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => Script::Arabic,
        0x590..=0x5FF => Script::Hebrew,
        0x900..=0x97F => Script::Devanagari,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FFFF => Script::Han,
        0x3040..=0x309F => Script::Hiragana,
        0x30A0..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9F => Script::Katakana,
        0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF => Script::Hangul,
        0xE00..=0xE7F => Script::Thai,
        0xE80..=0xEFF => Script::Lao,
        0x1780..=0x17FF => Script::Khmer,
        0x1000..=0x109F => Script::Myanmar,
        _ => Script::Common,
    }
}

/// A Latin letter outside ASCII — the census's measure of accents and the
/// like, by block.
#[must_use]
pub fn is_accented_latin(c: char) -> bool {
    !c.is_ascii()
        && c.is_alphabetic()
        && script(c) == Script::Latin
        && !matches!(u32::from(c), 0xFF21..=0xFF5A)
}

/// A compatibility form NFKC would change (D4), by block: full-width and
/// half-width forms, the Latin ligatures, super- and subscripts.
#[must_use]
pub fn is_compatibility(c: char) -> bool {
    matches!(
        u32::from(c),
        0xFF01..=0xFFEF | 0xFB00..=0xFB06 | 0x2070..=0x209F | 0xB2 | 0xB3 | 0xB9
    )
}

/// A Greek letter's name, any case, with or without its tonos or
/// dialytika; the micro sign is mu. `None` for anything else.
#[must_use]
pub fn greek_name(c: char) -> Option<&'static str> {
    let lower = c.to_lowercase().next().unwrap_or(c);
    Some(match lower {
        'α' | 'ά' => "alpha",
        'β' | 'ϐ' => "beta",
        'γ' => "gamma",
        'δ' => "delta",
        'ε' | 'έ' | 'ϵ' => "epsilon",
        'ζ' => "zeta",
        'η' | 'ή' => "eta",
        'θ' | 'ϑ' => "theta",
        'ι' | 'ί' | 'ϊ' | 'ΐ' => "iota",
        'κ' | 'ϰ' => "kappa",
        'λ' => "lambda",
        'μ' | 'µ' => "mu",
        'ν' => "nu",
        'ξ' => "xi",
        'ο' | 'ό' => "omicron",
        'π' | 'ϖ' => "pi",
        'ρ' | 'ϱ' => "rho",
        'σ' | 'ς' | 'ϲ' => "sigma",
        'τ' => "tau",
        'υ' | 'ύ' | 'ϋ' | 'ΰ' | 'ϒ' => "upsilon",
        'φ' | 'ϕ' => "phi",
        'χ' => "chi",
        'ψ' => "psi",
        'ω' | 'ώ' => "omega",
        _ => return None,
    })
}

/// D5: a term's Greek letters spelled as their names when the term is a
/// single Greek letter or mixes Greek letters with Latin letters or
/// digits (`α`, `δ13c`); a Greek word — two or more Greek letters and
/// nothing else — stays as it is.
#[must_use]
pub fn greek_named(term: &str) -> Cow<'_, str> {
    let greek = term.chars().filter(|c| greek_name(*c).is_some()).count();
    if greek == 0 {
        return Cow::Borrowed(term);
    }
    let other = term.chars().any(|c| greek_name(c).is_none());
    if greek > 1 && !other {
        return Cow::Borrowed(term);
    }
    let mut out = String::with_capacity(term.len() + 8 * greek);
    for c in term.chars() {
        match greek_name(c) {
            Some(name) => out.push_str(name),
            None => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// The hyphens D6 joins across.
const HYPHENS: [char; 4] = ['-', '\u{2010}', '\u{2011}', '\u{2013}'];

/// Each stretch of `text` where two of its `runs` meet across a single
/// hyphen (`IL-6`), as D6 would join them; `runs` as [`runs`] gives them.
pub fn hyphenated<'a>(
    text: &'a str,
    runs: &'a [(usize, &'a str)],
) -> impl Iterator<Item = &'a str> + 'a {
    runs.windows(2).filter_map(move |pair| {
        let [(a_at, a), (b_at, b)] = pair else {
            return None;
        };
        let mut gap = text[a_at + a.len()..*b_at].chars();
        match (gap.next(), gap.next()) {
            (Some(c), None) if HYPHENS.contains(&c) => Some(&text[*a_at..b_at + b.len()]),
            _ => None,
        }
    })
}

/// What a run splits between: digits, or letters by their script.
fn class(c: char) -> (bool, Script) {
    if c.is_numeric() {
        (true, Script::Common)
    } else {
        (false, script(c))
    }
}

/// A run's parts, split where letters and digits meet and where the
/// script changes (`IL6` is `IL` and `6`; `TNFα` is `TNF` and `α`); one
/// part when there is no such place.
#[must_use]
pub fn parts(run: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut from = 0;
    let mut last = None;
    for (at, c) in run.char_indices() {
        let class = class(c);
        if last.is_some_and(|l| l != class) {
            out.push(&run[from..at]);
            from = at;
        }
        last = Some(class);
    }
    out.push(&run[from..]);
    out
}

/// Whether a run has more than one part, without collecting them.
#[must_use]
pub fn is_split(run: &str) -> bool {
    let mut chars = run.chars().map(class);
    let first = chars.next();
    chars.any(|c| Some(c) != first)
}

/// D6, as the text's side of a match: each pair of runs joined across a
/// single hyphen (`IL-6` gives `il6`), and each run's parts when it has
/// more than one (`IL6` gives `il` and `6`), lowercased; so that `IL-6`
/// and `IL6`, either one typed and either one written, share a term.
#[must_use]
pub fn joins_and_splits(text: &str) -> Vec<String> {
    let runs = runs(text);
    let mut out: Vec<String> = hyphenated(text, &runs)
        .map(|span| {
            span.chars()
                .filter(|c| !HYPHENS.contains(c))
                .flat_map(char::to_lowercase)
                .collect()
        })
        .collect();
    for (_, run) in &runs {
        if is_split(run) {
            out.extend(parts(run).into_iter().map(str::to_lowercase));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_is_the_default_analyzer() {
        let mut a = Analyzers::default();
        assert_eq!(
            a.terms("TNF-α and IL-6"),
            vec!["tnf", "α", "and", "il", "6"]
        );
        assert!(a.terms("新型冠状病毒肺炎患者的临床特征分析").is_empty());
        assert!(
            a.terms("pneumonoultramicroscopicsilicovolcanoconiosis")
                .is_empty()
        );
        assert_eq!(
            a.uncapped("Pneumonoultramicroscopicsilicovolcanoconiosis"),
            vec!["pneumonoultramicroscopicsilicovolcanoconiosis"]
        );
    }

    #[test]
    fn runs_are_what_the_tokenizer_splits() {
        let text = "Étude de l'efficacité, IL-6";
        let got: Vec<&str> = runs(text).into_iter().map(|(_, r)| r).collect();
        assert_eq!(got, vec!["Étude", "de", "l", "efficacité", "IL", "6"]);
        for (at, run) in runs(text) {
            assert_eq!(&text[at..at + run.len()], run);
        }
    }

    #[test]
    fn scripts_by_block() {
        assert_eq!(script('a'), Script::Latin);
        assert_eq!(script('é'), Script::Latin);
        assert_eq!(script('α'), Script::Greek);
        assert_eq!(script('µ'), Script::Greek);
        assert_eq!(script('肺'), Script::Han);
        assert_eq!(script('に'), Script::Hiragana);
        assert_eq!(script('ラ'), Script::Katakana);
        assert_eq!(script('한'), Script::Hangul);
        assert_eq!(script('ก'), Script::Thai);
        assert_eq!(script('д'), Script::Cyrillic);
        assert_eq!(script('7'), Script::Common);
        assert!(Script::Han.unspaced() && !Script::Hangul.unspaced());
    }

    #[test]
    fn accents_and_compatibility_forms() {
        assert!(is_accented_latin('é') && is_accented_latin('ß') && !is_accented_latin('e'));
        assert!(!is_accented_latin('Ａ'));
        assert!(is_compatibility('Ａ') && is_compatibility('ﬁ') && is_compatibility('²'));
        assert!(!is_compatibility('A'));
    }

    /// Every letter of the basic Greek alphabet, capital and small, has a
    /// name: the table is checked against the block, not against the
    /// letters remembered.
    #[test]
    fn every_basic_greek_letter_has_a_name() {
        for code in (0x391..=0x3A9).chain(0x3B1..=0x3C9) {
            if code == 0x3A2 {
                continue; // unassigned: no capital final sigma
            }
            let c = char::from_u32(code).unwrap();
            assert!(greek_name(c).is_some(), "U+{code:04X} {c}");
        }
    }

    #[test]
    fn greek_letters_named_only_beside_other_letters() {
        assert_eq!(greek_named("α"), "alpha");
        assert_eq!(greek_named("δ13c"), "delta13c");
        assert_eq!(greek_named("tnfα"), "tnfalpha");
        assert_eq!(greek_named("λόγος"), "λόγος");
        assert_eq!(greek_named("lactamase"), "lactamase");
    }

    #[test]
    fn folding_and_stemming() {
        let mut a = Analyzers::default();
        assert_eq!(a.folded("étude"), "etude");
        assert_eq!(a.folded("müller"), "muller");
        assert_eq!(a.folded("ｄｎａ"), "dna");
        assert_eq!(a.stemmed("tumours"), "tumour");
        assert_eq!(a.stemmed("modelling"), a.stemmed("modeling"));
    }

    #[test]
    fn joins_and_splits_meet_either_spelling() {
        let dash = joins_and_splits("IL-6 levels");
        assert!(dash.contains(&"il6".to_owned()), "{dash:?}");
        let joined = joins_and_splits("IL6 levels");
        assert!(
            joined.contains(&"il".to_owned()) && joined.contains(&"6".to_owned()),
            "{joined:?}"
        );
        assert!(joins_and_splits("plain words only").is_empty());
        assert_eq!(parts("TNFα"), vec!["TNF", "α"]);
        assert_eq!(parts("DNA修复"), vec!["DNA", "修复"]);
        assert_eq!(parts("δ13C"), vec!["δ", "13", "C"]);
        assert_eq!(parts("étude"), vec!["étude"]);
        let text = "TNF-α, β-lactamase – x";
        let runs = runs(text);
        let spans: Vec<&str> = hyphenated(text, &runs).collect();
        assert_eq!(spans, vec!["TNF-α", "β-lactamase"]);
        assert!(joins_and_splits("TNF‐α").contains(&"tnfα".to_owned()));
    }

    use proptest::prelude::*;

    /// Text from any script, with the punctuation and long runs T0 counts.
    fn text() -> impl Strategy<Value = String> {
        prop_oneof![
            any::<String>(),
            "[a-zA-Zé0-9α-ωΣ肺癌がんกา한국ＤＮＡﬁ² .,;:()\\-]{0,120}",
        ]
    }

    proptest! {
        /// The census counts runs, not terms: for any text, the terms
        /// the index makes are its runs under the cap, lowercased as
        /// Tantivy lowercases (a character at a time).
        #[test]
        fn terms_are_the_runs_under_the_cap(text in text()) {
            let mut a = Analyzers::default();
            let runs: Vec<String> = runs(&text)
                .into_iter()
                .filter(|(_, r)| r.len() < CAP)
                .map(|(_, r)| r.chars().flat_map(char::to_lowercase).collect())
                .collect();
            prop_assert_eq!(a.terms(&text), runs);
        }

        /// A run's parts put back together are the run, and `is_split`
        /// says whether there is more than one.
        #[test]
        fn parts_make_the_run(text in text()) {
            for (_, run) in runs(&text) {
                let parts = parts(run);
                prop_assert_eq!(parts.concat(), run);
                prop_assert_eq!(is_split(run), parts.len() > 1, "{:?}", run);
            }
        }
    }
}
