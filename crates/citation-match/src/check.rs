//! The check (citation lookup plan §3, step 3; D4): does a citation say
//! what a work says?
//!
//! Five fields are looked for: the title, the first author, the year, the
//! volume and the first page. A work passes when every one of them it has
//! is found in the citation — **all of them, or it fails** (the user,
//! 2026-09-27: "all matches or none does"). A field the work does not have
//! counts for nothing either way, since absence is not agreement (the
//! merge's rule, 2026-09-18); and a work with none of the five has nothing
//! to agree with and does not pass.
//!
//! "Found" means exactly found, as Unicode compares text
//! ([`crate::text`]): the title as whole words somewhere in the citation;
//! a surname of the first author in the first name of its author list;
//! the year, and a numeric volume or page, among its numbers; any other
//! volume or page as whole words. The rule is loosened only by rules
//! found in the near misses (D4), each named ([`Rule`]). Two are, both on
//! the user's word of 2026-09-28 over lookups that missed: a title is
//! compared without the full stop that closes it, and a citation that is
//! a work's title and nothing more passes on the title alone.
//!
//! Numbers are read as Crossref's matcher reads them (its
//! `cr_search_validation_matcher.py`, MIT): every run of digits, a page
//! range written short read long (`265-75` is 265 and 275), and a volume
//! that is its year found by that one number. Each number accounts for
//! one field. What no field accounts for is listed, and counts against
//! the score, never against the rule: without a parser, an extra number
//! cannot be told from an access date or an issue the row does not hold.
//!
//! The score orders works; it is not the rule. It is what agreed over
//! everything either side mentions: the five, the issue and the last page
//! where the work has them, and every number left over.

use crate::identifiers;
use crate::text::{caseless, contains_words, words};

/// What a work says, as the check reads it: the fields of its served row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Work<'a> {
    #[allow(missing_docs)]
    pub title: Option<&'a str>,
    /// The first author as the row prints the name: one string, in
    /// whatever order its source gave, since no source's family name
    /// reaches the row yet (the plan's §12, item 4).
    pub first_author: Option<&'a str>,
    #[allow(missing_docs)]
    pub year: Option<u16>,
    #[allow(missing_docs)]
    pub volume: Option<&'a str>,
    #[allow(missing_docs)]
    pub issue: Option<&'a str>,
    /// As the sources write them: `265-275`, `265-75`, `e3000385`.
    pub pages: Option<&'a str>,
}

/// One field of a work, against the citation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agreement {
    /// The citation says what the work says.
    Agrees,
    /// The work says it and the citation does not.
    NotInCitation,
    /// The work does not say, so there is nothing to find.
    WorkDoesNotSay,
}

/// What the check found: each of the five fields, the citation's numbers
/// the work does not account for, and the score.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    #[allow(missing_docs)]
    pub title: Agreement,
    #[allow(missing_docs)]
    pub first_author: Agreement,
    #[allow(missing_docs)]
    pub year: Agreement,
    #[allow(missing_docs)]
    pub volume: Agreement,
    #[allow(missing_docs)]
    pub first_page: Agreement,
    /// Numbers in the citation that nothing the work says accounts for, in
    /// the order they stand.
    pub unaccounted: Vec<String>,
    /// From 0 to 1, for ordering: what agreed over everything mentioned.
    pub score: f64,
    /// The rule the work passes by, when it passes.
    pub rule: Option<Rule>,
}

/// A rule a work passes the check by: D4's, and each loosening of it,
/// named so that an answer says which let a work through and each can be
/// measured apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// Every one of the five the work has agrees, and it has one (D4).
    EveryField,
    /// The citation is the work's title and nothing more, closing full
    /// stops aside: a title typed on its own names its work, whatever
    /// else the work says, since the citation says nothing else at all
    /// (the user, 2026-09-28, on two titles pasted alone that missed).
    TitleAlone,
}

impl Rule {
    /// The rule's name, as an answer gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::EveryField => "every-field",
            Self::TitleAlone => "title-alone",
        }
    }
}

impl Verdict {
    /// The five, by name, in the order the plan lists them.
    #[must_use]
    pub fn fields(&self) -> [(&'static str, Agreement); 5] {
        [
            ("title", self.title),
            ("first_author", self.first_author),
            ("year", self.year),
            ("volume", self.volume),
            ("first_page", self.first_page),
        ]
    }

    /// Whether the work passes, by any [`Rule`].
    #[must_use]
    pub const fn passes(&self) -> bool {
        self.rule.is_some()
    }
}

/// A citation, read once to be checked against many works.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    /// As compared, with the identifiers found in it and a leading label
    /// taken out.
    text: String,
    /// The first name of its author list, as compared.
    first_name: String,
    /// Every number in it, in the order they stand.
    numbers: Vec<String>,
}

impl Citation {
    /// Reads `text`. Any text is read.
    ///
    /// The identifiers in it are taken out first: a DOI's digits are not
    /// the work's volume or pages, and the names are looked up on their
    /// own ([`identifiers`]). Then a leading label — `12.`, `[12]`,
    /// `(12)` — which numbers the list, not the work.
    #[must_use]
    pub fn read(text: &str) -> Self {
        let text = caseless(&searchable(text));
        let first_name = first_name(&text).to_owned();
        let numbers = numbers(&text);
        Self {
            text,
            first_name,
            numbers,
        }
    }

    /// The numbers the citation carries, in the order they stand.
    #[must_use]
    pub fn numbers(&self) -> &[String] {
        &self.numbers
    }
}

/// What of a citation says something about its work: the text without the
/// identifiers found in it, which are looked up by name, and the labels and
/// links they were written with ([`identifiers::Found::written`]); and
/// without a leading label, which numbers the list. Both what the check
/// reads and what a search for candidates is asked (citation lookup plan
/// §3, step 2), as written — the search's own analyzer folds it as the
/// index was folded.
#[must_use]
pub fn searchable(text: &str) -> String {
    let written = identifiers::find(text)
        .iter()
        .map(|found| found.written(text))
        .collect::<Vec<_>>();
    without_label(&identifiers::without(text, &written)).to_owned()
}

/// `text` without a leading label: a number in brackets, or followed by a
/// full stop or a closing parenthesis. A number followed by neither, as in
/// `2019 Annual report`, is the citation's own and stays.
fn without_label(text: &str) -> &str {
    let t = text.trim_start();
    let (open, rest) = match t.as_bytes().first() {
        Some(&b @ (b'[' | b'(')) => (Some(b), &t[1..]),
        _ => (None, t),
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return text;
    }
    let after = &rest[digits..];
    match (open, after.as_bytes().first()) {
        (Some(b'['), Some(b']')) | (Some(b'('), Some(b')')) | (None, Some(b'.' | b')')) => {
            &after[1..]
        }
        _ => text,
    }
}

/// The first name of the author list: the text up to its first comma,
/// semicolon or opening bracket. `Lowry OH`, `D. Tkaczyk`, `Lowry`.
fn first_name(text: &str) -> &str {
    let end = text.find([',', ';', '(', '[']).unwrap_or(text.len());
    &text[..end]
}

/// The dashes a page range is written with: Crossref's list.
fn is_dash(c: char) -> bool {
    matches!(
        c,
        '-' | '\u{ad}' | '\u{2010}'..='\u{2015}' | '\u{207b}' | '\u{208b}' | '\u{2212}'
    )
}

/// The second of a range written short, written long: `75` after `265` is
/// `275`, as Crossref reads it — when it has fewer digits than the first,
/// and the first's last digits are not above it. Otherwise itself.
fn long_form(first: &str, last: &str) -> String {
    if first.len() > last.len() {
        let tail = &first[first.len() - last.len()..];
        if tail.parse::<u64>().ok() <= last.parse::<u64>().ok() {
            return format!("{}{last}", &first[..first.len() - last.len()]);
        }
    }
    last.to_owned()
}

/// Every run of ASCII digits in `text`, in order, with the second of a
/// range written short (`265-75`) written long.
fn numbers(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut previous: Option<(usize, usize)> = None;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let run = &text[start..i];
        let ranged = previous.is_some_and(|(_, end)| {
            let mut between = text[end..start].chars();
            between.next().is_some_and(is_dash) && between.next().is_none()
        });
        out.push(match previous {
            Some((from, end)) if ranged => long_form(&text[from..end], run),
            _ => run.to_owned(),
        });
        previous = Some((start, i));
    }
    out
}

/// The first and last page of `pages` as a source writes them: before the
/// first dash, and after it, written long when written short. `345--348`
/// is 345 and 348; `e3000385` has no last page.
fn pages(pages: &str) -> (Option<String>, Option<String>) {
    let pages = pages.trim();
    let Some(at) = pages.find(is_dash) else {
        return ((!pages.is_empty()).then(|| pages.to_owned()), None);
    };
    let first = pages[..at].trim();
    let last = pages[at..].trim_start_matches(is_dash).trim();
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let last = if digits(first) && digits(last) {
        long_form(first, last)
    } else {
        last.to_owned()
    };
    (
        (!first.is_empty()).then(|| first.to_owned()),
        (!last.is_empty()).then_some(last),
    )
}

/// Takes one `number` from `left`, if it is there.
fn take(left: &mut Vec<String>, number: &str) -> bool {
    match left.iter().position(|n| n == number) {
        Some(at) => {
            left.remove(at);
            true
        }
        None => false,
    }
}

/// Whether the citation says `value`: a number among its numbers, taken so
/// no other field counts it; anything else as whole words in its text,
/// whose numbers are then accounted for.
fn says(value: Option<&str>, left: &mut Vec<String>, citation: &str) -> Agreement {
    let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Agreement::WorkDoesNotSay;
    };
    if value.bytes().all(|b| b.is_ascii_digit()) {
        return found(take(left, value));
    }
    let value = caseless(value);
    let agrees = contains_words(citation, &value);
    if agrees {
        for n in numbers(&value) {
            take(left, &n);
        }
    }
    found(agrees)
}

fn found(yes: bool) -> Agreement {
    if yes {
        Agreement::Agrees
    } else {
        Agreement::NotInCitation
    }
}

/// Whether the first author's family name is in the first name of the
/// citation's author list. The row prints a name given names first —
/// `Oliver H. Lowry`, `Alex G C de Sá` — so its family name is taken as its
/// last word, or, where the printed name has a comma (`Lowry, Oliver H.`),
/// the last word before it (the plan's §12, item 4). Any word at all was
/// taken until 2026-09-28, when `Chinese Society of Health Management`
/// agreed with a citation by its `of`. A family name of one letter is an
/// initial, and says nothing to look for.
fn author(name: Option<&str>, first_name: &str) -> Agreement {
    let Some(name) = name else {
        return Agreement::WorkDoesNotSay;
    };
    let name = caseless(name);
    let family_part = name
        .split_once(',')
        .map_or(name.as_str(), |(before, _)| before);
    let Some(family) = words(family_part).last().filter(|w| w.chars().count() > 1) else {
        return Agreement::WorkDoesNotSay;
    };
    found(words(first_name).any(|w| w == family))
}

/// A title as compared: without the full stop that closes it. PubMed's
/// titles carry one, and a citation may close the title with a comma or a
/// quote instead (the user, 2026-09-28: "we can drop the period at the end
/// of a title").
fn title_form(title: &str) -> String {
    let title = caseless(title);
    match title.strip_suffix('.') {
        Some(bare) => bare.trim_end().to_owned(),
        None => title,
    }
}

/// Checks `work` against `citation`.
#[must_use]
pub fn check(citation: &Citation, work: &Work<'_>) -> Verdict {
    let text = citation.text.as_str();
    let mut left = citation.numbers.clone();

    let title_text = work.title.map(title_form).filter(|t| !t.is_empty());
    let title = match &title_text {
        Some(title) => found(contains_words(text, title)),
        None => Agreement::WorkDoesNotSay,
    };
    let title_alone = title_text
        .as_deref()
        .is_some_and(|title| title_form(text) == title);
    let first_author = author(work.first_author, &citation.first_name);

    // The five first, so that a number only the issue or the last page
    // could claim is never taken from a field the rule reads.
    let year_text = work.year.map(|y| y.to_string());
    let year = says(year_text.as_deref(), &mut left, text);
    let mut volume = says(work.volume, &mut left, text);
    // A volume numbered by its year is found by the one number: Crossref's
    // reading, for journals that number a volume a year.
    if volume == Agreement::NotInCitation
        && year == Agreement::Agrees
        && work.volume.map(str::trim) == year_text.as_deref()
    {
        volume = Agreement::Agrees;
    }
    let (first, last) = work.pages.map(pages).unwrap_or_default();
    let first_page = says(first.as_deref(), &mut left, text);
    let issue = says(work.issue, &mut left, text);
    let last_page = says(last.as_deref(), &mut left, text);
    // The title's own numbers are the work's too: `COVID-19`, `p53`.
    if title == Agreement::Agrees
        && let Some(t) = work.title
    {
        for n in numbers(&caseless(t)) {
            take(&mut left, &n);
        }
    }

    let counted = [
        title,
        first_author,
        year,
        volume,
        first_page,
        issue,
        last_page,
    ];
    let agreed = counted.iter().filter(|a| **a == Agreement::Agrees).count();
    let mentioned = counted
        .iter()
        .filter(|a| **a != Agreement::WorkDoesNotSay)
        .count()
        + left.len();
    #[allow(clippy::cast_precision_loss)] // A handful of fields and numbers.
    let score = if mentioned == 0 {
        0.0
    } else {
        agreed as f64 / mentioned as f64
    };
    let five = [title, first_author, year, volume, first_page];
    let every_field =
        five.contains(&Agreement::Agrees) && !five.contains(&Agreement::NotInCitation);
    let rule = if every_field {
        Some(Rule::EveryField)
    } else if title_alone {
        Some(Rule::TitleAlone)
    } else {
        None
    };
    Verdict {
        title,
        first_author,
        year,
        volume,
        first_page,
        unaccounted: left,
        score,
        rule,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::Agreement::{Agrees, NotInCitation, WorkDoesNotSay};
    use super::{Citation, Rule, Verdict, Work, check, numbers, pages};

    fn checked(citation: &str, work: &Work<'_>) -> Verdict {
        check(&Citation::read(citation), work)
    }

    /// The plan's example A and B.
    const LOWRY_A: &str = "Lowry OH, Rosebrough NJ, Farr AL, Randall RJ. Protein \
        measurement with the Folin phenol reagent. J Biol Chem. 1951;193(1):265-75.";
    const LOWRY_B: &str = "Lowry OH, Rosebrough NJ, Farr AL, Randall RJ (1951) J Biol Chem 193:265";

    /// Lowry's paper as a row will have it once the citation fields land.
    const LOWRY: Work<'static> = Work {
        title: Some("Protein measurement with the Folin phenol reagent"),
        first_author: Some("Oliver H. Lowry"),
        year: Some(1951),
        volume: Some("193"),
        issue: Some("1"),
        pages: Some("265-275"),
    };

    /// And as a row has it today: no volume, issue or pages.
    const LOWRY_TODAY: Work<'static> = Work {
        volume: None,
        issue: None,
        pages: None,
        ..LOWRY
    };

    #[test]
    fn example_a_passes_on_all_five_with_nothing_left_over() {
        let v = checked(LOWRY_A, &LOWRY);
        assert!(v.passes());
        assert!(v.fields().iter().all(|(_, a)| *a == Agrees), "{v:?}");
        assert!(v.unaccounted.is_empty(), "{v:?}");
        assert!((v.score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn example_a_passes_today_on_three_with_its_numbers_unaccounted() {
        let v = checked(LOWRY_A, &LOWRY_TODAY);
        assert!(v.passes());
        assert_eq!(
            (v.title, v.first_author, v.year, v.volume, v.first_page),
            (Agrees, Agrees, Agrees, WorkDoesNotSay, WorkDoesNotSay)
        );
        assert_eq!(v.unaccounted, ["193", "1", "265", "275"]);
        assert!((v.score - 3.0 / 7.0).abs() < 1e-9, "{}", v.score);
    }

    #[test]
    fn example_b_is_a_near_miss_on_its_title_alone() {
        let v = checked(LOWRY_B, &LOWRY);
        assert!(!v.passes());
        assert_eq!(
            (v.title, v.first_author, v.year, v.volume, v.first_page),
            (NotInCitation, Agrees, Agrees, Agrees, Agrees)
        );
        assert!(v.unaccounted.is_empty());
        // The issue and last page are not in it: they count against.
        assert!((v.score - 4.0 / 7.0).abs() < 1e-9, "{}", v.score);
    }

    /// Crossref's own cases for its matcher
    /// (`matching/tests/test_cr_search_validation_matcher.py`, MIT, ©
    /// Crossref): the CERMINE paper cited in IEEE's style, with and
    /// without its title, and a sentence that is not a citation.
    const CERMINE: Work<'static> = Work {
        title: Some(
            "CERMINE: automatic extraction of structured metadata from scientific literature",
        ),
        first_author: Some("Dominika Tkaczyk"),
        year: Some(2015),
        volume: Some("18"),
        issue: Some("4"),
        pages: Some("317-335"),
    };

    #[test]
    fn crossref_s_ieee_citation_passes() {
        let v = checked(
            "[1]D. Tkaczyk, P. Szostek, M. Fedoryszak, P. J. Dendek, and Ł. \
             Bolikowski,“CERMINE: automatic extraction of structured metadata \
             from scientific literature,” International Journal on Document \
             Analysis and Recognition (IJDAR), vol. 18, no. 4, pp. 317–335, 2015.",
            &CERMINE,
        );
        assert!(v.passes(), "{v:?}");
        assert!(v.unaccounted.is_empty(), "{v:?}");
    }

    /// Crossref matches this one on its other four fields. Here it fails
    /// on its title, as example B does, until a rule of ours says a
    /// citation without a title may pass (the plan's D4).
    #[test]
    fn crossref_s_citation_without_its_title_is_a_near_miss_here() {
        let v = checked(
            "[1] D. Tkaczyk, P. Szostek, M. Fedoryszak, P.J. Dendek, Ł. \
             Bolikowski, International Journal on Document Analysis and \
             Recognition (IJDAR) 18 (2015) 317.",
            &CERMINE,
        );
        assert!(!v.passes());
        assert_eq!(
            (v.title, v.first_author, v.year, v.volume, v.first_page),
            (NotInCitation, Agrees, Agrees, Agrees, Agrees)
        );
    }

    #[test]
    fn crossref_s_sentence_that_is_no_citation_fails() {
        let v = checked(
            "Nosek recently lead a project in which 270 scientists sought to \
             replicate 100 different studies in psychology, all published in \
             2008 — 97 of which claimed to have found significant results — \
             and in the end, two-thirds failed to replicate.",
            &CERMINE,
        );
        assert!(!v.passes());
        assert_eq!(
            (v.title, v.first_author, v.year),
            (NotInCitation, NotInCitation, NotInCitation)
        );
    }

    #[test]
    fn crossref_s_readings_of_pages() {
        assert_eq!(pages("345--348"), (Some("345".into()), Some("348".into())));
        assert_eq!(pages("265-75"), (Some("265".into()), Some("275".into())));
        assert_eq!(pages("e3000385"), (Some("e3000385".into()), None));
        assert_eq!(pages("S12–S15"), (Some("S12".into()), Some("S15".into())));
        assert_eq!(
            numbers("pp. 1425-37 and 90–9"),
            ["1425", "1437", "90", "99"]
        );
        // Not a range written short: the second is not above the tail.
        assert_eq!(numbers("1999-20"), ["1999", "20"]);
    }

    #[test]
    fn the_year_is_exact() {
        let v = checked(&LOWRY_A.replace("1951", "1952"), &LOWRY);
        assert_eq!(v.year, NotInCitation);
        assert!(!v.passes());
    }

    #[test]
    fn a_title_is_found_as_unicode_compares_text_and_as_whole_words() {
        let work = |title| Work {
            title: Some(title),
            ..Work::default()
        };
        let title = |citation: &str, title| checked(citation, &work(title)).title;
        assert_eq!(
            title("X. STRASSE UND VERKEHR. 2001.", "Straße und Verkehr"),
            Agrees
        );
        assert_eq!(
            title("X. Cafe\u{301} society. 2001.", "Café society"),
            Agrees
        );
        // Unicode folds the ligature, its capital being `FI`.
        assert_eq!(title("X. ﬁbre optics. 2001.", "Fibre optics"), Agrees);
        // Not yet: accents, full-width letters, and part of a word.
        assert_eq!(
            title("X. Cafe society. 2001.", "Café society"),
            NotInCitation
        );
        assert_eq!(
            title("X. ｆｉｂｒｅ optics. 2001.", "Fibre optics"),
            NotInCitation
        );
        assert_eq!(title("X. Cancers of the lung.", "Cancer"), NotInCitation);
    }

    #[test]
    fn a_surname_is_looked_for_in_the_first_name_and_an_initial_is_none() {
        let by = |author| Work {
            first_author: Some(author),
            ..Work::default()
        };
        let author = |citation: &str, name| checked(citation, &by(name)).first_author;
        assert_eq!(
            author(
                "Lowry, O. H., & Rosebrough, N. J. (1951).",
                "Oliver H. Lowry"
            ),
            Agrees
        );
        assert_eq!(author("12. Lowry OH, Farr AL.", "Oliver H. Lowry"), Agrees);
        assert_eq!(author("Lowry OH, Farr AL.", "Lowry, Oliver H."), Agrees);
        // The family name, not any word: `of` is no one's surname.
        assert_eq!(
            author(
                "Trends and multidisciplinary research of torch ginger.",
                "Chinese Society of Health Management"
            ),
            NotInCitation
        );
        assert_eq!(
            author(
                "Chinese Society of Health Management. Consensus.",
                "Chinese Society of Health Management"
            ),
            Agrees
        );
        // The second author's surname is not the first's.
        assert_eq!(
            author("Jones A, Smith B. A title.", "B Smith"),
            NotInCitation
        );
        // `A` is an initial in both, and no surname.
        assert_eq!(
            author("A. Jones, B. Smith. A title.", "A Smith"),
            NotInCitation
        );
        assert_eq!(author("A. Jones. A title.", "A B"), WorkDoesNotSay);
    }

    #[test]
    fn a_label_and_an_identifier_are_not_the_work_s_numbers() {
        let cited = Citation::read(
            "12. Lowry OH. Protein measurement with the Folin phenol reagent. \
             J Biol Chem. 1951;193(1):265-75. doi:10.1016/S0021-9258(19)52451-6. \
             PMID: 14907713",
        );
        assert_eq!(cited.numbers(), ["1951", "193", "1", "265", "275"]);
        // A number that is no label stays.
        assert_eq!(
            Citation::read("2019 Annual report of the society.").numbers(),
            ["2019"]
        );
    }

    #[test]
    fn a_volume_numbered_by_its_year_is_found_by_the_one_number() {
        let work = Work {
            year: Some(2015),
            volume: Some("2015"),
            pages: Some("12"),
            ..Work::default()
        };
        let v = checked("Smith J. A title. J Tests 2015:12.", &work);
        assert_eq!((v.year, v.volume, v.first_page), (Agrees, Agrees, Agrees));
    }

    #[test]
    fn a_page_that_is_no_number_is_found_as_a_word_and_its_digits_accounted() {
        let work = Work {
            year: Some(2019),
            volume: Some("17"),
            issue: Some("10"),
            pages: Some("e3000385"),
            ..Work::default()
        };
        let v = checked(
            "Hutchins BI, et al. PLoS Biol. 2019;17(10):e3000385.",
            &work,
        );
        assert_eq!(v.first_page, Agrees);
        assert!(v.unaccounted.is_empty(), "{v:?}");
    }

    /// PubMed's titles close with a full stop, and IEEE closes a title with
    /// a comma inside quotes (found on the dev store, 2026-09-27).
    #[test]
    fn a_title_s_closing_full_stop_is_not_asked_for() {
        let work = Work {
            title: Some("Aspirin hypersensitivity: a practical guide for cardiologists."),
            first_author: Some("Silvia Grimaldi"),
            year: Some(2024),
            ..Work::default()
        };
        let v = checked(
            "S. Grimaldi, P. Migliorini, and I. Puxeddu, “Aspirin hypersensitivity: \
             a practical guide for cardiologists,” Eur. Heart J., 2024.",
            &work,
        );
        assert_eq!(v.title, Agrees);
        assert_eq!(v.rule, Some(Rule::EveryField));
    }

    /// A title typed on its own names its work (the two that missed on
    /// 2026-09-28), with or without its full stop, in any case — and only
    /// the whole of it, and nothing more.
    #[test]
    fn a_citation_that_is_a_title_alone_passes_on_its_title() {
        let work = Work {
            title: Some("Trends in biodiversity research in Brazilian marine protected areas."),
            first_author: Some("Vinicius J Giglio"),
            year: Some(2026),
            ..Work::default()
        };
        for alone in [
            "Trends in biodiversity research in Brazilian marine protected areas.",
            "trends in biodiversity research in brazilian marine protected areas",
            "  TRENDS IN BIODIVERSITY RESEARCH IN BRAZILIAN MARINE PROTECTED AREAS  ",
        ] {
            let v = checked(alone, &work);
            assert_eq!(v.rule, Some(Rule::TitleAlone), "{alone:?}");
            // The author and year are still said not to be in it.
            assert_eq!((v.first_author, v.year), (NotInCitation, NotInCitation));
        }
        for more_or_less in [
            "Trends in biodiversity research in Brazilian marine protected areas. 2025.",
            "Trends in biodiversity research in Brazilian marine areas.",
        ] {
            assert!(!checked(more_or_less, &work).passes(), "{more_or_less:?}");
        }
    }

    #[test]
    fn a_work_with_nothing_to_check_does_not_pass() {
        let v = checked(LOWRY_A, &Work::default());
        assert!(v.fields().iter().all(|(_, a)| *a == WorkDoesNotSay));
        assert!(!v.passes());
        assert!(v.score.abs() < f64::EPSILON);
    }

    /// A work's fields, owned: title, first author, year, volume, issue,
    /// pages.
    type Owned = (
        Option<String>,
        Option<String>,
        Option<u16>,
        Option<String>,
        Option<String>,
        Option<String>,
    );

    /// A work's fields, drawn small: words, and numbers under a million.
    fn work() -> impl Strategy<Value = Owned> {
        (
            prop::option::of("[A-Za-zé ]{1,30}"),
            prop::option::of("[A-Za-z. ]{1,20}"),
            prop::option::of(1900_u16..2030),
            prop::option::of(prop_oneof!["[0-9]{1,5}", "[A-Za-z0-9 ]{1,8}"]),
            prop::option::of("[0-9]{1,3}"),
            prop::option::of(prop_oneof![
                "[0-9]{1,5}",
                "[0-9]{1,5}-[0-9]{1,5}",
                "e[0-9]{1,6}"
            ]),
        )
    }

    fn as_work((title, author, year, volume, issue, pages): &Owned) -> Work<'_> {
        Work {
            title: title.as_deref(),
            first_author: author.as_deref(),
            year: *year,
            volume: volume.as_deref(),
            issue: issue.as_deref(),
            pages: pages.as_deref(),
        }
    }

    /// Citations weighted towards what the check reads: numbers, ranges,
    /// labels, names, punctuation, and any character at all.
    fn citation() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop_oneof![
                2 => any::<char>().prop_map(String::from),
                3 => "[ -~]{1,6}",
                2 => "[0-9]{1,6}",
                1 => prop::sample::select(vec![
                    "-", "–", "[1]", "12. ", ", ", "; ", "(", ")", "doi:10.1000/x ", "PMID: 5 ",
                    "Lowry", "Straße", "é", "\u{301}", "ﬁ",
                ])
                .prop_map(String::from),
            ],
            0..40,
        )
        .prop_map(|pieces| pieces.concat())
    }

    proptest! {
        /// Any citation is read and any work checked. The score is between
        /// 0 and 1, and what is left over is the citation's own numbers.
        #[test]
        fn anything_is_checked(text in citation(), fields in work()) {
            let cited = Citation::read(&text);
            let v = check(&cited, &as_work(&fields));
            prop_assert!((0.0..=1.0).contains(&v.score), "{}", v.score);
            for n in &v.unaccounted {
                prop_assert!(cited.numbers().contains(n), "{:?} is not the citation's", n);
            }
        }

        /// A number the work does not have, added to the citation, changes
        /// no field and passes nothing, and never raises the score.
        #[test]
        fn an_extra_number_fails_nothing_and_raises_nothing(
            text in citation(),
            fields in work(),
        ) {
            let work = as_work(&fields);
            let before = checked(&text, &work);
            let after = checked(&format!("{text} 98765432"), &work);
            prop_assert_eq!(before.fields(), after.fields());
            prop_assert!(after.score <= before.score + 1e-12);
        }

        /// Absence is not agreement: a field taken away from a work is one
        /// the work does not say, and no other field goes from agreeing to
        /// not — taking a field away can only leave a number free.
        #[test]
        fn a_field_taken_away_agrees_with_nothing(text in citation(), fields in work()) {
            let full = as_work(&fields);
            let before = checked(&text, &full);
            let without: [(Work<'_>, usize); 5] = [
                (Work { title: None, ..full }, 0),
                (Work { first_author: None, ..full }, 1),
                (Work { year: None, ..full }, 2),
                (Work { volume: None, ..full }, 3),
                (Work { pages: None, ..full }, 4),
            ];
            for (less, gone) in without {
                let after = checked(&text, &less);
                for (n, ((name, was), (_, now))) in
                    before.fields().iter().zip(after.fields().iter()).enumerate()
                {
                    if n == gone {
                        prop_assert_eq!(*now, WorkDoesNotSay, "{}", name);
                    } else {
                        prop_assert!(
                            !(*was == Agrees && *now == NotInCitation),
                            "{} went from agreeing to not", name
                        );
                    }
                }
            }
        }

        /// A title typed in capitals is the same title.
        #[test]
        fn a_title_in_capitals_is_the_same_title(
            title in "[a-z]{2,8}( [a-z]{2,8}){0,5}",
            before in "[A-Z][a-z]{1,8} [A-Z]{1,2}\\. ",
            after in "\\. [A-Z][a-z]{1,8} [0-9]{4}\\.",
        ) {
            let work = Work { title: Some(&title), ..Work::default() };
            let lower = checked(&format!("{before}{title}{after}"), &work);
            let upper = checked(&format!("{before}{}{after}", title.to_uppercase()), &work);
            prop_assert_eq!(lower.title, Agrees);
            prop_assert_eq!(upper.title, Agrees);
        }
    }
}
