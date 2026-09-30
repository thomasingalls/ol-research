//! Text as the check compares it (citation lookup plan, D11): Unicode's own
//! answer to "is this the same text, case aside?", and nothing more.
//!
//! Two spellings of one text compare equal when Unicode's canonical
//! caseless match says they do (The Unicode Standard §3.13, D145): each is
//! decomposed, case-folded and decomposed again. `é` typed as one
//! character or as `e` and an accent is one `é`; `STRASSE` is `straße`;
//! `Protein Measurement` is `protein measurement`. Here the result is
//! composed again (NFC), which keeps that equality and keeps an accented
//! letter one character, so that `cafe` is not found inside `café`.
//!
//! Folding is Unicode's, and does more than lowercase: `ß` folds to `ss`,
//! and the ligature `ﬁ` to `fi`, since its capital is `FI` — the
//! standard's tables say so, not a choice made here.
//!
//! What is *not* done, on purpose: accents are not dropped (`Muller` is not
//! `Müller`), compatibility forms folding leaves alone are not folded
//! (full-width `ｆｉｂｒｅ` is not `fibre`), punctuation is not ignored,
//! nothing is transliterated. Each is a rule to be found in the near misses
//! and measured, not assumed (the user, 2026-09-27: "we loosen by
//! discovering sensible rules as we go").
//! Runs of whitespace are one space, since a citation pasted from a PDF
//! breaks wherever its column did.
//!
//! Every table here is Unicode's, through ICU4X — the Unicode Consortium's
//! Rust ICU — and none is written by hand.

use icu_casemap::CaseMapper;
use icu_normalizer::{ComposingNormalizer, DecomposingNormalizer};
use icu_properties::CodePointMapData;
use icu_properties::props::{GeneralCategory, GeneralCategoryGroup};

/// `text` as the check compares it: canonically decomposed, case-folded,
/// composed, and each run of whitespace one space, none at either end.
#[must_use]
pub fn caseless(text: &str) -> String {
    let decomposed = DecomposingNormalizer::new_nfd().normalize(text);
    let folded = CaseMapper::new().fold_string(&decomposed);
    let composed = ComposingNormalizer::new_nfc().normalize(&folded);
    let mut out = String::with_capacity(composed.len());
    for piece in composed.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(piece);
    }
    out
}

/// Whether `c` is part of a word: a letter, a mark or a number, by
/// Unicode's general categories. A mark, so that an accent no composed
/// letter carries does not split its word in two.
#[must_use]
pub fn in_word(c: char) -> bool {
    let category = CodePointMapData::<GeneralCategory>::new().get(c);
    GeneralCategoryGroup::Letter.contains(category)
        || GeneralCategoryGroup::Mark.contains(category)
        || GeneralCategoryGroup::Number.contains(category)
}

/// The words of `text`: its runs of word characters ([`in_word`]).
pub fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !in_word(c)).filter(|w| !w.is_empty())
}

/// Whether `needle` stands in `haystack` as whole words: found, with no
/// word character just before it or just after. `cancer` is not in
/// `cancers of the lung`; `e3000385` is in `17(10):e3000385.`. An empty
/// needle is in nothing.
#[must_use]
pub fn contains_words(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack.match_indices(needle).any(|(at, _)| {
        let before = haystack[..at].chars().next_back();
        let after = haystack[at + needle.len()..].chars().next();
        !before.is_some_and(in_word) && !after.is_some_and(in_word)
    })
}

#[cfg(test)]
mod tests {
    use icu_normalizer::{ComposingNormalizer, DecomposingNormalizer};
    use proptest::prelude::*;

    use super::{caseless, contains_words, words};

    #[test]
    fn case_is_folded_as_unicode_folds_it() {
        assert_eq!(caseless("Protein Measurement"), "protein measurement");
        // Full folding, not lowercasing: ß and SS are one text.
        assert_eq!(caseless("STRASSE"), caseless("Straße"));
        assert_eq!(caseless("ΣΟΦΊΑ"), caseless("σοφία"));
        // The ligature's capital is `FI`, so Unicode folds it to `fi`.
        assert_eq!(caseless("ﬁbre"), "fibre");
    }

    #[test]
    fn one_letter_typed_two_ways_is_one_letter() {
        assert_eq!(caseless("Cafe\u{301}"), caseless("Café"));
        assert_eq!(caseless("Cafe\u{301}").chars().count(), 4);
    }

    #[test]
    fn nothing_else_is_folded_yet() {
        // Accents, ligatures and punctuation are rules to be found, not
        // assumed (the module's note).
        assert_ne!(caseless("Cafe"), caseless("Café"));
        assert_ne!(caseless("ｆｉｂｒｅ"), caseless("fibre"));
        assert_ne!(caseless("Folin-phenol"), caseless("Folin phenol"));
    }

    #[test]
    fn whitespace_is_one_space() {
        assert_eq!(
            caseless("  Protein\nmeasurement \u{a0}with\tthe  "),
            "protein measurement with the"
        );
    }

    #[test]
    fn a_word_keeps_its_accents_and_whole_words_are_whole() {
        assert_eq!(
            words("[1]D. Tkaczyk, Ł. Bolikowski").collect::<Vec<_>>(),
            ["1", "D", "Tkaczyk", "Ł", "Bolikowski"]
        );
        // `i` and a combining dot, which no composed letter is: one word.
        assert_eq!(words(&caseless("İstanbul")).count(), 1);
        assert!(contains_words("17(10):e3000385.", "e3000385"));
        assert!(!contains_words("cancers of the lung", "cancer"));
        assert!(!contains_words("anything", ""));
    }

    proptest! {
        /// Comparing twice is comparing once.
        #[test]
        fn the_compare_form_is_its_own(text in any::<String>()) {
            let once = caseless(&text);
            prop_assert_eq!(caseless(&once), once);
        }

        /// A text typed composed or decomposed compares the same: they are
        /// one text by Unicode's canonical equivalence.
        #[test]
        fn two_encodings_of_one_text_compare_equal(text in any::<String>()) {
            let composed = ComposingNormalizer::new_nfc().normalize(&text).into_owned();
            let decomposed = DecomposingNormalizer::new_nfd().normalize(&text).into_owned();
            prop_assert_eq!(caseless(&composed), caseless(&decomposed));
        }
    }
}
