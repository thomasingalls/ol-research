//! The identifiers written in a citation (citation lookup plan §3, step
//! 1; D2): a DOI, a PMID, a PMC id, found where they stand in the text and
//! read into the names the names table is keyed by — `doi:10.1000/x`,
//! `pmid:12345678`, `pmc:PMC1234567`.
//!
//! People write them every way there is: `https://doi.org/10.1038/…`,
//! `doi:10.1038/…`, a bare `10.1038/…`, in capitals, with a soft hyphen a
//! PDF left behind; `PMID: 14907713`, `PMID 14907713`, a PubMed link;
//! `PMCID: PMC1234567`. Each DOI is read the way the model reads one
//! ([`model::Doi`]), so what is found here is spelled as the names table
//! spells it.
//!
//! Nothing here looks a name up. A DOI in running text has no clear end:
//! `…/nature12373.` is a DOI and the full stop of its sentence, and
//! `…(19)52451-6` carries parentheses of its own. So a DOI is found as
//! every name it may be, longest first — each after the first with one
//! more character taken off the end that a sentence may have put there —
//! and the caller takes the first its table holds.
//!
//! No arXiv ids: the names table has no arXiv namespace, so one is not an
//! identifier here (the plan's §12, item 7).

use std::ops::Range;

use model::{Doi, OpenAlexId, PmcId, Pmid};

/// Which kind of identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A DOI.
    Doi,
    /// A PubMed id.
    Pmid,
    /// A PubMed Central id.
    Pmc,
    /// An OpenAlex work id, found by its link.
    OpenAlex,
}

/// One way to read an identifier: the name, and how much of the text it
/// was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    /// The name in the names table's text form: `doi:10.1000/x`.
    pub name: String,
    /// Where it stands, in bytes: `&text[span]` is what was read.
    pub span: Range<usize>,
}

/// One identifier, where it stands in the citation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    #[allow(missing_docs)]
    pub kind: Kind,
    /// The names it may be, longest reading first; never empty. More than
    /// one only for a DOI whose end is in doubt, and then each starts
    /// where the first does and ends a character or more short of it.
    pub readings: Vec<Reading>,
}

impl Found {
    /// The whole of what was found, in bytes: the longest reading's span.
    #[must_use]
    pub fn span(&self) -> Range<usize> {
        self.readings[0].span.clone()
    }

    /// Where it stands in `text` — the text it was found in — with the
    /// label or link it was written with: `doi:`, `https://doi.org/`,
    /// `PMCID: `, `https://www.ncbi.nlm.nih.gov/pmc/articles/` before it,
    /// and a link's closing `/` after. None of that says anything of the
    /// work, so none of it is a citation's words.
    #[must_use]
    pub fn written(&self, text: &str) -> Range<usize> {
        let span = self.span();
        let mut start = span.start;
        'more: loop {
            for label in LABELS {
                if ends_with_folded(&text.as_bytes()[..start], label) {
                    start -= label.len();
                    continue 'more;
                }
            }
            break;
        }
        let end = if text.as_bytes().get(span.end) == Some(&b'/') {
            span.end + 1
        } else {
            span.end
        };
        start..end
    }
}

/// `text` with each of `ranges` — byte ranges of it, overlapping or not —
/// written as a single space.
#[must_use]
pub fn without(text: &str, ranges: &[Range<usize>]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut inside = false;
    for (at, c) in text.char_indices() {
        if ranges.iter().any(|r| r.contains(&at)) {
            if !inside {
                out.push(' ');
                inside = true;
            }
        } else {
            out.push(c);
            inside = false;
        }
    }
    out
}

/// What an identifier is introduced by, lowercase: links and labels, taken
/// off one after another, so that `https://`, `www.` and
/// `ncbi.nlm.nih.gov/pmc/articles/` add up to a whole PMC link. The longer
/// of two that end alike comes first.
const LABELS: &[&str] = &[
    "https://",
    "http://",
    "www.",
    "dx.doi.org/",
    "doi.org/",
    "doi: ",
    "doi:",
    "doi ",
    "pmcid: ",
    "pmcid:",
    "pmcid ",
    "pubmed ",
    "ncbi.nlm.nih.gov/pmc/articles/",
    "pmc.ncbi.nlm.nih.gov/articles/",
];

/// Whether `bytes` ends with `label`, ASCII case aside.
fn ends_with_folded(bytes: &[u8], label: &str) -> bool {
    bytes.len() >= label.len()
        && bytes[bytes.len() - label.len()..].eq_ignore_ascii_case(label.as_bytes())
}

/// Every identifier written in `text`, in the order they stand there.
/// Any text is read; none is refused. Two never overlap: a PMID or PMC id
/// inside a DOI (`10.1000/pmc12345`) is part of the DOI.
#[must_use]
pub fn find(text: &str) -> Vec<Found> {
    // ASCII lowercasing moves no byte, so a place found in the folded
    // text is the same place in the text as written.
    let folded = text.to_ascii_lowercase();
    let mut found = dois(text);
    let others = pmcs(text, &folded)
        .into_iter()
        .chain(pmids(text, &folded))
        .chain(openalex(text, &folded));
    for other in others {
        let span = other.span();
        if found.iter().all(|f| !overlaps(&f.span(), &span)) {
            found.push(other);
        }
    }
    found.sort_by_key(|f| f.span().start);
    found
}

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

// --- DOIs ----------------------------------------------------------------

/// Where a DOI written in running text ends, at the latest: whitespace,
/// or the quotes and angle brackets that wrap one. None of them is a
/// character of a DOI the model reads, so nothing is lost by stopping.
fn ends_doi(c: char) -> bool {
    c.is_whitespace() || matches!(c, '"' | '<' | '>' | '“' | '”' | '‘' | '’' | '«' | '»')
}

fn dois(text: &str) -> Vec<Found> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find("10.").map(|i| from + i) {
        from = at + 3;
        // `110.1000/x` is no DOI; `doi:10.`, `/10.` and ` 10.` are.
        if at > 0 && bytes[at - 1].is_ascii_digit() {
            continue;
        }
        let registrant = bytes[at + 3..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        let slash = at + 3 + registrant;
        if !(4..=9).contains(&registrant) || bytes.get(slash) != Some(&b'/') {
            continue;
        }
        let suffix = slash + 1;
        let end = text[suffix..]
            .find(ends_doi)
            .map_or(text.len(), |n| suffix + n);
        let readings = doi_readings(text, at, suffix, end);
        if !readings.is_empty() {
            out.push(Found {
                kind: Kind::Doi,
                readings,
            });
            from = end;
        }
    }
    out
}

/// The names `text[start..end]` may be, longest first: the whole run,
/// then with a trailing `.`, `,`, `;`, `:` or `'` taken off, or a closing
/// bracket the run never opened, one character at a time until the end
/// is none of those or the suffix would be empty. Left out: a reading the
/// model refuses; one it does not read back as itself, which the names
/// table would refuse as not canonical; and one that reads as the same
/// name as a longer one.
fn doi_readings(text: &str, start: usize, suffix: usize, end: usize) -> Vec<Reading> {
    let mut out: Vec<Reading> = Vec::new();
    let mut end = end;
    while end > suffix {
        let written = &text[start..end];
        if let Some(doi) = canonical(written) {
            let name = format!("doi:{doi}");
            if out.iter().all(|r| r.name != name) {
                out.push(Reading {
                    name,
                    span: start..end,
                });
            }
        }
        let Some(last) = written.chars().next_back() else {
            break;
        };
        let sentence = match last {
            '.' | ',' | ';' | ':' | '\'' => true,
            ')' => unopened(written, '(', ')'),
            ']' => unopened(written, '[', ']'),
            '}' => unopened(written, '{', '}'),
            _ => false,
        };
        if !sentence {
            break;
        }
        end -= last.len_utf8();
    }
    out
}

/// The DOI `written` reads as, when the model reads that back as itself:
/// the names table holds a DOI only in the form the model gives it, and
/// refuses any other as not canonical.
fn canonical(written: &str) -> Option<String> {
    let doi = Doi::parse(written).ok()?;
    let again = Doi::parse(doi.as_str()).ok()?;
    (again.as_str() == doi.as_str()).then(|| doi.as_str().to_owned())
}

/// Whether `written` closes more brackets than it opens: the bracket
/// belongs to the sentence around the DOI, as in `(doi:10.1000/x)`, and
/// not to the DOI, as in `…(19)52451-6`.
fn unopened(written: &str, open: char, close: char) -> bool {
    written.matches(close).count() > written.matches(open).count()
}

// --- PMIDs and PMC ids ---------------------------------------------------

/// Whether `at` starts a word: the start of the text, or after a
/// character that is not an ASCII letter or digit. `xpmid` is not a PMID
/// label; `[pmid`, ` pmid` and `/pmid` are.
fn starts_word(bytes: &[u8], at: usize) -> bool {
    at == 0 || !bytes[at - 1].is_ascii_alphanumeric()
}

/// The run of ASCII digits at `at`, and where it ends.
fn digits_at(text: &str, at: usize) -> (&str, usize) {
    let n = text.as_bytes()[at..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    (&text[at..at + n], at + n)
}

/// Every place `needle`, lowercase ASCII, stands in `folded`.
fn places<'a>(folded: &'a str, needle: &'a str) -> impl Iterator<Item = usize> + 'a {
    folded.match_indices(needle).map(|(at, _)| at)
}

/// A PMID written with its label — `PMID: 14907713`, `PMID 14907713`,
/// `pmid:14907713` — or as a PubMed link, `pubmed.ncbi.nlm.nih.gov/14907713`
/// or the older `ncbi.nlm.nih.gov/pubmed/14907713`. A bare number is never
/// taken for a PMID: a citation is full of numbers.
fn pmids(text: &str, folded: &str) -> Vec<Found> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    for at in places(folded, "pmid") {
        if !starts_word(bytes, at) {
            continue;
        }
        let mut next = at + 4;
        while bytes.get(next) == Some(&b' ') {
            next += 1;
        }
        if matches!(bytes.get(next), Some(b':' | b'=' | b'#')) {
            next += 1;
        }
        while bytes.get(next) == Some(&b' ') {
            next += 1;
        }
        out.extend(pmid(text, at, next));
    }
    // The two links cannot find one PMID twice: the newer host has no
    // `/pubmed/` after it, and the older has no `pubmed.` before it.
    for link in ["pubmed.ncbi.nlm.nih.gov/", "ncbi.nlm.nih.gov/pubmed/"] {
        for at in places(folded, link) {
            out.extend(pmid(text, at, at + link.len()));
        }
    }
    out
}

/// The PMID whose digits start at `digits`, written from `at` on.
fn pmid(text: &str, at: usize, digits: usize) -> Option<Found> {
    let (written, end) = digits_at(text, digits);
    let pmid = Pmid::from_digits(written).ok()?;
    Some(Found {
        kind: Kind::Pmid,
        readings: vec![Reading {
            name: format!("pmid:{pmid}"),
            span: at..end,
        }],
    })
}

/// An OpenAlex work by its link, `openalex.org/W2741809807`. A bare `W…`
/// in running text is not taken for one, as a bare number is not taken
/// for a PMID.
fn openalex(text: &str, folded: &str) -> Vec<Found> {
    const LINK: &str = "openalex.org/";
    places(folded, LINK)
        .filter_map(|at| {
            let from = at + LINK.len();
            let letter = text.as_bytes().get(from)?;
            if !letter.eq_ignore_ascii_case(&b'w') {
                return None;
            }
            let (written, end) = digits_at(text, from + 1);
            let id = OpenAlexId::from_bare(&format!("W{written}")).ok()?;
            Some(Found {
                kind: Kind::OpenAlex,
                readings: vec![Reading {
                    name: format!("openalex:{id}"),
                    span: at..end,
                }],
            })
        })
        .collect()
}

/// Every name one token typed on its own, with no namespace, may be (the
/// user, 2026-09-28: "i can't expect the general public to be able to come
/// up with 'reg1016'"): itself in every namespace whose parser takes it —
/// a DOI, a PMID, a PMC id with its `PMC` or without, an OpenAlex work id
/// in either case, a public id without its `ol:`, and a publisher's own id
/// in each of `publishers` (`reg1016`, …), which only the names table
/// knows. `12345` is both `pmid:12345` and `pmc:PMC12345`, and both are
/// offered.
///
/// Only for a token: never for text with a space or a colon in it, where
/// the namespaces are written or the identifiers are [`find`]'s to find.
/// A name offered here may name nothing, and one the table refuses is
/// simply no name.
#[must_use]
pub fn bare(token: &str, publishers: &[String]) -> Vec<String> {
    let token = token.trim();
    if token.is_empty() || token.contains(char::is_whitespace) || token.contains(':') {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(doi) = canonical(token) {
        out.push(format!("doi:{doi}"));
    }
    if let Ok(pmid) = Pmid::from_digits(token) {
        out.push(format!("pmid:{pmid}"));
    }
    let pmc_digits = match token.get(..3) {
        Some(prefix) if prefix.eq_ignore_ascii_case("pmc") => &token[3..],
        _ => token,
    };
    if let Ok(pmc) = PmcId::from_digits(pmc_digits) {
        out.push(format!("pmc:{pmc}"));
    }
    if let Ok(work) = OpenAlexId::from_bare(&token.to_ascii_uppercase()) {
        out.push(format!("openalex:{work}"));
    }
    out.push(format!("ol:{token}"));
    for namespace in publishers {
        out.push(format!("{namespace}:{token}"));
    }
    out
}

/// A PMC id: `PMC` and its digits, in any case, as a word of its own —
/// `PMC1234567`, `PMCID: PMC1234567`, `…/pmc/articles/PMC1234567/`.
fn pmcs(text: &str, folded: &str) -> Vec<Found> {
    let bytes = text.as_bytes();
    places(folded, "pmc")
        .filter(|&at| starts_word(bytes, at))
        .filter_map(|at| {
            let (written, end) = digits_at(text, at + 3);
            let pmc = PmcId::from_digits(written).ok()?;
            Some(Found {
                kind: Kind::Pmc,
                readings: vec![Reading {
                    // `PmcId` prints with its prefix: `PMC1234567`.
                    name: format!("pmc:{pmc}"),
                    span: at..end,
                }],
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{Kind, bare, find};

    /// Each identifier found, as the names it may be.
    fn names(text: &str) -> Vec<Vec<String>> {
        find(text)
            .iter()
            .map(|f| f.readings.iter().map(|r| r.name.clone()).collect())
            .collect()
    }

    /// The Lowry paper as a reference list writes it (the plan's example
    /// A), with its DOI and the full stop that ends the citation.
    const LOWRY: &str = "Lowry OH, Rosebrough NJ, Farr AL, Randall RJ. Protein \
        measurement with the Folin phenol reagent. J Biol Chem. \
        1951;193(1):265-75. doi:10.1016/S0021-9258(19)52451-6.";

    #[test]
    fn a_doi_keeps_its_own_brackets_and_offers_the_full_stop_last() {
        let found = find(LOWRY);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Doi);
        assert_eq!(
            names(LOWRY),
            [[
                "doi:10.1016/s0021-9258(19)52451-6.",
                "doi:10.1016/s0021-9258(19)52451-6",
            ]]
        );
        let bare = &found[0].readings[1];
        assert_eq!(&LOWRY[bare.span.clone()], "10.1016/S0021-9258(19)52451-6");
    }

    #[test]
    fn a_bracket_the_doi_never_opened_is_the_sentence_s() {
        assert_eq!(
            names("(doi:10.1000/xyz)."),
            [["doi:10.1000/xyz).", "doi:10.1000/xyz)", "doi:10.1000/xyz"]]
        );
        assert_eq!(
            names("[10.1000/xyz]"),
            [["doi:10.1000/xyz]", "doi:10.1000/xyz"]]
        );
    }

    #[test]
    fn a_doi_is_read_the_way_the_model_reads_one() {
        for (text, name) in [
            (
                "https://doi.org/10.1038/NATURE12373",
                "doi:10.1038/nature12373",
            ),
            ("DOI 10.1038/nature12373", "doi:10.1038/nature12373"),
            // JAMA's soft hyphen after the slash (model 11).
            (
                "doi:10.1001/\u{ad}jamasurg.2022.7640",
                "doi:10.1001/jamasurg.2022.7640",
            ),
            // Typographic quotes end a DOI; they are no part of one.
            ("“10.1000/xyz”", "doi:10.1000/xyz"),
            ("<https://doi.org/10.1000/xyz>", "doi:10.1000/xyz"),
        ] {
            assert_eq!(names(text), [[name]], "{text:?}");
        }
    }

    #[test]
    fn a_number_shaped_like_part_of_a_doi_is_not_one() {
        for text in [
            // A volume and page.
            "Phys Rev 112:1169 (1958)",
            // Three digits of registrant, a digit before `10.`, no suffix.
            "vol 10.123/x",
            "110.1000/x",
            "10.1000/",
            "10.1000",
        ] {
            assert!(find(text).is_empty(), "{text:?}: {:?}", names(text));
        }
    }

    /// The model lowercases `İ` to `i` and a combining dot, and refuses
    /// that dot when it reads the result again (2026-09-27), so no name
    /// the table could hold is spelled that way: nothing is found rather
    /// than a name the table refuses.
    #[test]
    fn a_doi_the_model_does_not_read_back_is_not_offered() {
        assert!(find("10.1000/İ").is_empty(), "{:?}", names("10.1000/İ"));
        assert_eq!(names("10.1000/Σ"), [["doi:10.1000/σ"]]);
    }

    #[test]
    fn a_pmid_is_found_by_its_label_or_its_link_and_never_bare() {
        for text in [
            "Lowry OH et al. J Biol Chem. 1951. PMID: 14907713",
            "PMID:14907713.",
            "[pmid 14907713]",
            "PubMed PMID: 14907713",
            "https://pubmed.ncbi.nlm.nih.gov/14907713/",
            "https://www.ncbi.nlm.nih.gov/pubmed/14907713",
        ] {
            assert_eq!(names(text), [["pmid:14907713"]], "{text:?}");
        }
        for text in ["J Biol Chem 193:265", "14907713", "PMID: 0123", "xpmid 5"] {
            assert!(find(text).is_empty(), "{text:?}: {:?}", names(text));
        }
    }

    #[test]
    fn a_pmc_id_is_found_as_a_word_of_its_own() {
        for text in [
            "PMCID: PMC3737249",
            "pmc3737249",
            "https://www.ncbi.nlm.nih.gov/pmc/articles/PMC3737249/",
        ] {
            assert_eq!(names(text), [["pmc:PMC3737249"]], "{text:?}");
        }
        assert!(find("XPMC3737249").is_empty());
    }

    #[test]
    fn several_are_found_in_the_order_they_stand() {
        let text = "Nature 2013. doi:10.1038/nature12373. PMID: 23851394; PMCID: PMC3737249.";
        let found = find(text);
        assert_eq!(
            found.iter().map(|f| f.kind).collect::<Vec<_>>(),
            [Kind::Doi, Kind::Pmid, Kind::Pmc]
        );
        assert_eq!(found[1].readings[0].name, "pmid:23851394");
        assert_eq!(found[2].readings[0].name, "pmc:PMC3737249");
    }

    #[test]
    fn an_openalex_work_is_found_by_its_link_and_never_bare_in_text() {
        assert_eq!(
            names("https://openalex.org/W2741809807"),
            [["openalex:W2741809807"]]
        );
        assert_eq!(names("openalex.org/w123."), [["openalex:W123"]]);
        assert!(find("W2741809807").is_empty());
    }

    #[test]
    fn a_bare_token_is_offered_in_every_namespace_that_takes_it() {
        let publishers = ["reg1016".to_owned(), "reg1053".to_owned()];
        assert_eq!(
            bare("40975432", &publishers),
            [
                "pmid:40975432",
                "pmc:PMC40975432",
                "ol:40975432",
                "reg1016:40975432",
                "reg1053:40975432"
            ]
        );
        assert_eq!(bare("PMC3737249", &[]), ["pmc:PMC3737249", "ol:PMC3737249"]);
        assert_eq!(
            bare("w2741809807", &[]),
            ["openalex:W2741809807", "ol:w2741809807"]
        );
        assert_eq!(
            bare("10.1038/NATURE12373", &[])[0],
            "doi:10.1038/nature12373"
        );
        assert_eq!(
            bare("S0022522325007792", &publishers),
            [
                "ol:S0022522325007792",
                "reg1016:S0022522325007792",
                "reg1053:S0022522325007792"
            ]
        );
        // Written with its namespace, or more than one token: not bare.
        assert!(bare("pmid:123", &publishers).is_empty());
        assert!(bare("Lowry 1951", &publishers).is_empty());
    }

    /// What an identifier was written with is part of it, and no word of
    /// the citation's.
    #[test]
    fn an_identifier_is_written_with_its_label_or_link() {
        for (text, written) in [
            ("https://doi.org/10.1000/x", "https://doi.org/10.1000/x"),
            (
                "See http://dx.doi.org/10.1000/x.",
                "http://dx.doi.org/10.1000/x.",
            ),
            ("Nature. DOI: 10.1000/x", "DOI: 10.1000/x"),
            ("PubMed PMID: 14907713", "PubMed PMID: 14907713"),
            ("PMCID: PMC3737249", "PMCID: PMC3737249"),
            (
                "https://www.ncbi.nlm.nih.gov/pmc/articles/PMC3737249/",
                "https://www.ncbi.nlm.nih.gov/pmc/articles/PMC3737249/",
            ),
            (
                "https://pubmed.ncbi.nlm.nih.gov/14907713/",
                "https://pubmed.ncbi.nlm.nih.gov/14907713/",
            ),
            (
                "https://openalex.org/W2741809807",
                "https://openalex.org/W2741809807",
            ),
        ] {
            let found = find(text);
            assert_eq!(found.len(), 1, "{text:?}");
            assert_eq!(&text[found[0].written(text)], written, "{text:?}");
        }
        // And what is left once it is taken out.
        let text = "Nature. DOI: 10.1000/x";
        let written = find(text)[0].written(text);
        assert_eq!(super::without(text, &[written]), "Nature.  ");
        assert_eq!(super::without("abcdef", &[1..3, 2..5]), "a f");
    }

    #[test]
    fn what_stands_inside_a_doi_is_the_doi_s() {
        assert_eq!(names("10.1000/pmc12345"), [["doi:10.1000/pmc12345"]]);
        assert_eq!(names("10.1000/x-pmid-5"), [["doi:10.1000/x-pmid-5"]]);
    }

    /// Text weighted towards what the finder looks for: pieces of DOIs,
    /// labels, digits, and the characters a PDF leaves in a DOI.
    fn text() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop_oneof![
                3 => any::<char>().prop_map(String::from),
                3 => "[ -~]{1,4}",
                1 => "[0-9]{1,10}",
                1 => prop::sample::select(vec![
                    "10.", "10.1000/", "doi:", "https://doi.org/", "PMID: ", "pmid", "PMC",
                    "pubmed.ncbi.nlm.nih.gov/", "(", ")", ".", "\u{ad}", "\u{200b}", "\u{a0}",
                    "“", "”", "é",
                ])
                .prop_map(String::from),
            ],
            0..40,
        )
        .prop_map(|pieces| pieces.concat())
    }

    proptest! {
        /// Any text is read. What is found stands where it says, in
        /// order and apart, and every name is one the merge's own parser
        /// takes — so the server never asks the names table a question
        /// it refuses.
        #[test]
        fn any_text_is_read_and_every_name_is_a_name(text in text()) {
            let mut after = 0;
            for found in find(&text) {
                prop_assert!(!found.readings.is_empty());
                let span = found.span();
                prop_assert!(span.start >= after, "out of order or overlapping");
                after = span.end;
                for reading in &found.readings {
                    prop_assert!(text.get(reading.span.clone()).is_some_and(|s| !s.is_empty()));
                    prop_assert_eq!(reading.span.start, span.start);
                    prop_assert!(reading.span.end <= span.end);
                    let parsed = merge::Name::parse(&reading.name);
                    prop_assert!(parsed.is_ok(), "{:?}: {:?}", reading.name, parsed);
                }
            }
        }

        /// A DOI set in any text, after a space, however it is labelled,
        /// in whatever case, and followed by what a sentence puts after
        /// one, is found as itself.
        #[test]
        fn a_doi_in_running_text_is_found(
            before in text(),
            label in prop::sample::select(vec!["", "doi:", "doi: ", "DOI ", "https://doi.org/"]),
            registrant in "[1-9][0-9]{3,8}",
            suffix in "[A-Za-z0-9]([A-Za-z0-9._/-]{0,30}[A-Za-z0-9])?",
            after in "[.,;:)\\]]{0,2}",
            rest in text(),
        ) {
            let doi = format!("10.{registrant}/{suffix}");
            let text = format!("{before} {label}{doi}{after} {rest}");
            let want = format!("doi:{}", doi.to_ascii_lowercase());
            prop_assert!(
                find(&text).iter().any(|f| f.readings.iter().any(|r| r.name == want)),
                "{:?} not found in {:?}", want, text
            );
        }

        /// A PMID set in any text with one of its labels is found.
        #[test]
        fn a_labelled_pmid_is_found(
            before in text(),
            label in prop::sample::select(vec![
                "PMID: ", "PMID:", "pmid ", "PubMed PMID: ", "https://pubmed.ncbi.nlm.nih.gov/",
            ]),
            pmid in 1_u32..=99_999_999,
            rest in text(),
        ) {
            let text = format!("{before} {label}{pmid} {rest}");
            let want = format!("pmid:{pmid}");
            prop_assert!(
                find(&text).iter().any(|f| f.readings.iter().any(|r| r.name == want)),
                "{:?} not found in {:?}", want, text
            );
        }

        /// And a PMC id.
        #[test]
        fn a_pmc_id_is_found(
            before in text(),
            label in prop::sample::select(vec![
                "PMC", "pmc", "PMCID: PMC", "https://www.ncbi.nlm.nih.gov/pmc/articles/PMC",
            ]),
            pmc in 1_u32..=99_999_999,
            rest in text(),
        ) {
            let text = format!("{before} {label}{pmc} {rest}");
            let want = format!("pmc:PMC{pmc}");
            prop_assert!(
                find(&text).iter().any(|f| f.readings.iter().any(|r| r.name == want)),
                "{:?} not found in {:?}", want, text
            );
        }
    }
}
