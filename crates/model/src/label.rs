//! A volume's or an issue's label, read (citation fields plan D12, model
//! version 13): text, as the source writes it, with the levels written
//! into it read out. `20 Suppl 2` in a volume is volume `20`, then
//! supplement `2`; `2 Pt 1` in an issue is issue `2`, then part `1`;
//! `Suppl` in an issue is a supplement and no issue. What is not a marker
//! stays as written: combined issues (`3-4`, `28/29`), `n° 229`, `2022.1`,
//! `[1977] VR`. 98.8% of PubMed's volumes and 95.4% of its issues are
//! plain digits, and 97.8% and 92.7% of Crossref's journal articles'
//! (`ol-rht-survey-placement-v1`, 2026-09-29); the rest carry what a
//! citation needs.
//!
//! The markers read are those the survey counted:
//! - a supplement: `Suppl`, `Supp`, `Supplement`, with a full stop or
//!   without, joined to what is beside it or not (`11suppl`, `3_suppl`,
//!   `Supplement_1`, `5_suppl_2`, `Supplement-1`), and an issue that is
//!   `S` and a number (`S6`, `s73`, `S 01`);
//! - a part: `Pt`, `Part` (`2 Pt 1`, `73 ( Pt 11)`, `PT1`, `3Pt2`,
//!   `Pt. 1`, `4 Part B`, `7S_Part_4`);
//! - a special issue: `Spec No`, `Spec Issue`, `Special Issue`,
//!   `Hors-série`;
//! - a series before a volume: `s2-XI`, `S5-X`, old journals' volumes
//!   numbered within a series (D13).
//!
//! Each marker's label is what follows it, up to the next marker: `1E` of
//! `17 Suppl 1E`. What reading `S6` or `s2-XI` supposes is returned beside
//! the label ([`Readings`]), to be counted as a page field's readings are.
//! A marker in a shape this reading does not know is kept as written
//! ([`Label::Kept`]) and counted by shape: `36 Suppl 2 Incretin`,
//! `22(9)Suppl`. A placeholder (`&NA;`, `Online first`) reads as no label.

use std::fmt;

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// A volume or an issue field, read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Label<'a> {
    /// No marker in it: the level's label as written, less the space
    /// around it (`45`, `3-4`, `n° 229`).
    Plain(&'a str),
    /// The levels written in it, outermost first, each with its label.
    Read(#[serde(borrow)] SmallVec<[Level<'a>; 2]>),
    /// A marker in a shape not read: the field as written, less the space
    /// around it.
    Kept(&'a str),
}

/// One level a label names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level<'a> {
    pub kind: LevelKind,
    /// As written, less its marker: `2` of `Suppl 2`, `XI` of `s2-XI`.
    /// `None` where the marker stands alone (`Suppl`, `Spec No`).
    #[serde(borrow)]
    pub label: Option<&'a str>,
}

/// The kinds of container a work sits in (the plan's D13). A label names
/// the journal's; the rest are the chain's, when it is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LevelKind {
    // A journal's.
    Journal,
    Series,
    Volume,
    Issue,
    Supplement,
    Part,
    SpecialIssue,
    // A book's, and proceedings', when they are filled (D9); a series
    // above either is `Series`.
    Book,
    Section,
    Chapter,
    Proceedings,
}

impl LevelKind {
    /// The kind, in words, for a report.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Journal => "journal",
            Self::Series => "series",
            Self::Volume => "volume",
            Self::Issue => "issue",
            Self::Supplement => "supplement",
            Self::Part => "part",
            Self::SpecialIssue => "special issue",
            Self::Book => "book",
            Self::Section => "section",
            Self::Chapter => "chapter",
            Self::Proceedings => "proceedings",
        }
    }

    /// The word a report writes before the label, NLM's where it has one;
    /// `None` for a volume or an issue, whose label stands alone.
    const fn marker(self) -> Option<&'static str> {
        match self {
            Self::Volume | Self::Issue => None,
            Self::Series => Some("Ser"),
            Self::Supplement => Some("Suppl"),
            Self::Part => Some("Pt"),
            Self::SpecialIssue => Some("Spec No"),
            Self::Journal => Some("Journal"),
            Self::Book => Some("Book"),
            Self::Section => Some("Sect"),
            Self::Chapter => Some("Chap"),
            Self::Proceedings => Some("Proc"),
        }
    }
}

/// What reading a label supposed beyond the markers written in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Readings {
    /// A placeholder was read as no label.
    pub placeholder: bool,
    /// An issue that is `S` and a number was read as a supplement.
    pub s_number: bool,
    /// A volume written after a series (`s2-XI`) was read as two levels.
    pub series: bool,
}

impl Readings {
    /// Each reading made, in words, for a report.
    pub fn made(self) -> impl Iterator<Item = &'static str> {
        [
            (self.placeholder, "a placeholder read as no label"),
            (self.s_number, "S and a number read as a supplement"),
            (self.series, "a series read before the volume"),
        ]
        .into_iter()
        .filter_map(|(made, what)| made.then_some(what))
    }
}

impl<'a> Label<'a> {
    /// Reads a volume or an issue field; `own` is the level it is for,
    /// [`LevelKind::Volume`] or [`LevelKind::Issue`]. `None` for nothing
    /// but space, or a placeholder.
    #[must_use]
    pub fn read(field: &'a str, own: LevelKind) -> (Option<Self>, Readings) {
        let mut readings = Readings::default();
        let text = field.trim();
        if text.is_empty() {
            return (None, readings);
        }
        if crate::pages::is_placeholder(text) {
            readings.placeholder = true;
            return (None, readings);
        }
        if own == LevelKind::Volume
            && let Some((series, volume)) = series(text)
        {
            readings.series = true;
            let levels =
                [(LevelKind::Series, series), (LevelKind::Volume, volume)].map(|(kind, label)| {
                    Level {
                        kind,
                        label: Some(label),
                    }
                });
            return (Some(Self::Read(SmallVec::from_buf(levels))), readings);
        }
        if own == LevelKind::Issue
            && let Some(number) = s_number(text)
        {
            readings.s_number = true;
            let level = Level {
                kind: LevelKind::Supplement,
                label: Some(number),
            };
            return (Some(Self::Read(SmallVec::from_elem(level, 1))), readings);
        }
        let marks = markers(text);
        if marks.is_empty() {
            return (Some(Self::Plain(text)), readings);
        }
        let label = levels(text, own, &marks).map_or(Self::Kept(text), Self::Read);
        (Some(label), readings)
    }

    /// The levels it names, outermost first: a plain or a kept label names
    /// `own`, the level the field is for.
    pub fn levels(&self, own: LevelKind) -> impl Iterator<Item = Level<'a>> + '_ {
        let (whole, read) = match self {
            Self::Plain(text) | Self::Kept(text) => (Some(*text), &[][..]),
            Self::Read(levels) => (None, &levels[..]),
        };
        whole
            .map(|text| Level {
                kind: own,
                label: Some(text),
            })
            .into_iter()
            .chain(read.iter().copied())
    }
}

impl fmt::Display for Label<'_> {
    /// For reports; a citation prints the levels its own way.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plain(text) | Self::Kept(text) => f.write_str(text),
            Self::Read(levels) => {
                for (i, level) in levels.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    match (level.kind.marker(), level.label) {
                        (Some(marker), Some(label)) => write!(f, "{marker} {label}")?,
                        (Some(marker), None) => f.write_str(marker)?,
                        (None, label) => f.write_str(label.unwrap_or_default())?,
                    }
                }
                Ok(())
            }
        }
    }
}

/// A marker found in a label: where it is, and the level it opens.
#[derive(Debug, Clone, Copy)]
struct Marker {
    start: usize,
    end: usize,
    kind: LevelKind,
}

/// The markers in a label, in the order written.
fn markers(text: &str) -> SmallVec<[Marker; 2]> {
    let words = words(text);
    let mut out = SmallVec::new();
    let mut i = 0;
    while i < words.len() {
        let (start, end) = words[i];
        let word = &text[start..end];
        // The word after, where only space, a full stop or a hyphen is
        // between: `Spec No`, `Spec. No.`, `Hors-série`.
        let next = words
            .get(i + 1)
            .filter(|&&(next, _)| {
                text[end..next]
                    .chars()
                    .all(|c| matches!(c, ' ' | '.' | '-'))
            })
            .map(|&(s, e)| &text[s..e]);
        let (kind, took) = if is(word, &["suppl", "supp", "supplement"]) {
            (Some(LevelKind::Supplement), 1)
        } else if is(word, &["pt", "part"]) {
            (Some(LevelKind::Part), 1)
        } else if (is(word, &["spec"]) && next.is_some_and(|n| is(n, &["no", "issue"])))
            || (is(word, &["special"]) && next.is_some_and(|n| is(n, &["issue"])))
            || (is(word, &["hors"]) && next.is_some_and(|n| is(n, &["série", "serie"])))
        {
            (Some(LevelKind::SpecialIssue), 2)
        } else {
            (None, 1)
        };
        if let Some(kind) = kind {
            out.push(Marker {
                start,
                end: words[i + took - 1].1,
                kind,
            });
        }
        i += took;
    }
    out
}

/// Where each run of letters starts and ends.
fn words(text: &str) -> SmallVec<[(usize, usize); 4]> {
    let mut out = SmallVec::new();
    let mut start = None;
    for (at, c) in text.char_indices() {
        match (c.is_alphabetic(), start) {
            (true, None) => start = Some(at),
            (false, Some(s)) => {
                out.push((s, at));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, text.len()));
    }
    out
}

/// Whether a word is one of `names`, which are lower case, in any case.
fn is(word: &str, names: &[&str]) -> bool {
    names
        .iter()
        .any(|name| word.chars().flat_map(char::to_lowercase).eq(name.chars()))
}

/// The levels a label with markers names, or `None` where one of them is
/// not a single label: `36 Suppl 2 Incretin`, `22(9)Suppl`.
fn levels<'a>(text: &'a str, own: LevelKind, marks: &[Marker]) -> Option<SmallVec<[Level<'a>; 2]>> {
    let mut out = SmallVec::new();
    if let Some(before) = between(&text[..marks[0].start]) {
        if !is_one(before) {
            return None;
        }
        out.push(Level {
            kind: own,
            label: Some(before),
        });
    }
    for (i, mark) in marks.iter().enumerate() {
        let end = marks.get(i + 1).map_or(text.len(), |next| next.start);
        let label = between(&text[mark.end..end]);
        if label.is_some_and(|l| !is_one(l)) {
            return None;
        }
        out.push(Level {
            kind: mark.kind,
            label,
        });
    }
    Some(out)
}

/// The text between two markers, less what joins it to them (space, `_`,
/// `-`, `.`, brackets); `None` where nothing is left.
fn between(piece: &str) -> Option<&str> {
    let piece =
        piece.trim_matches(|c: char| c.is_whitespace() || matches!(c, '_' | '-' | '.' | '(' | ')'));
    (!piece.is_empty()).then_some(piece)
}

/// Whether a label is one label: no space or bracket inside it.
fn is_one(label: &str) -> bool {
    !label
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']'))
}

/// A volume numbered within a series: `s2-XI` is series `2`, volume `XI`.
/// The survey counted 272,309 Crossref volumes of this shape, every
/// example a roman volume, so a volume in digits (`S1-2`) is not read.
fn series(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix(['s', 'S'])?;
    let (number, volume) = rest.split_once('-')?;
    let read = !number.is_empty()
        && number.bytes().all(|b| b.is_ascii_digit())
        && crate::pages::is_roman(volume);
    read.then_some((number, volume))
}

/// An issue that is `S` and a number: `S6`, `s73`, `S 01`.
fn s_number(text: &str) -> Option<&str> {
    let rest = text.strip_prefix(['s', 'S'])?;
    let number = rest.strip_prefix(' ').unwrap_or(rest);
    let read = !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit());
    read.then_some(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The levels read, as `kind label`, or `kept` or `none`.
    fn read(s: &str, own: LevelKind) -> String {
        match Label::read(s, own).0 {
            None => "none".into(),
            Some(Label::Kept(text)) => format!("kept {text}"),
            Some(label @ (Label::Plain(_) | Label::Read(_))) => label
                .levels(own)
                .map(|l| match l.label {
                    Some(label) => format!("{} {label}", l.kind.as_str()),
                    None => l.kind.as_str().to_owned(),
                })
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    fn volume(s: &str) -> String {
        read(s, LevelKind::Volume)
    }

    fn issue(s: &str) -> String {
        read(s, LevelKind::Issue)
    }

    #[test]
    fn plain() {
        assert_eq!(volume("13"), "volume 13");
        assert_eq!(issue(" 2 "), "issue 2");
        assert_eq!(issue("3-4"), "issue 3-4");
        assert_eq!(issue("28/29"), "issue 28/29");
        assert_eq!(volume("n° 229"), "volume n° 229");
        assert_eq!(volume("2022.1"), "volume 2022.1");
        assert_eq!(volume("[1977] VR"), "volume [1977] VR");
        assert_eq!(issue("Second Half"), "issue Second Half");
        assert_eq!(issue("1A"), "issue 1A");
        assert_eq!(issue("SP2"), "issue SP2");
        assert!(matches!(
            Label::read("45", LevelKind::Volume).0,
            Some(Label::Plain("45"))
        ));
    }

    #[test]
    fn supplements() {
        assert_eq!(volume("20 Suppl 2"), "volume 20, supplement 2");
        assert_eq!(volume("4 SUPPL 2"), "volume 4, supplement 2");
        assert_eq!(volume("23 suppl"), "volume 23, supplement");
        assert_eq!(volume("11suppl"), "volume 11, supplement");
        assert_eq!(volume("52suppl 1"), "volume 52, supplement 1");
        assert_eq!(volume("52(Suppl)"), "volume 52, supplement");
        assert_eq!(volume("17 Suppl 1E"), "volume 17, supplement 1E");
        assert_eq!(volume("Suppl 2"), "supplement 2");
        assert_eq!(issue("Suppl"), "supplement");
        assert_eq!(issue("Suppl. 1"), "supplement 1");
        assert_eq!(issue("Supplement_1"), "supplement 1");
        assert_eq!(issue("Supplement_D"), "supplement D");
        assert_eq!(issue("Supplement-1"), "supplement 1");
        assert_eq!(issue("3_suppl"), "issue 3, supplement");
        assert_eq!(issue("4_Suppl"), "issue 4, supplement");
        assert_eq!(issue("suppl_11"), "supplement 11");
        assert_eq!(issue("5_suppl_2"), "issue 5, supplement 2");
        assert_eq!(issue("4 Suppl 3"), "issue 4, supplement 3");
    }

    #[test]
    fn s_and_a_number() {
        assert_eq!(issue("S6"), "supplement 6");
        assert_eq!(issue("s73"), "supplement 73");
        assert_eq!(issue("S 01"), "supplement 01");
        assert!(Label::read("S6", LevelKind::Issue).1.s_number);
        // Only an issue's.
        assert_eq!(volume("S6"), "volume S6");
        assert_eq!(issue("S6A"), "issue S6A");
    }

    #[test]
    fn parts() {
        assert_eq!(issue("2 Pt 1"), "issue 2, part 1");
        assert_eq!(issue("Pt 1"), "part 1");
        assert_eq!(issue("PT1"), "part 1");
        assert_eq!(issue("3Pt2"), "issue 3, part 2");
        assert_eq!(issue("Pt. 1"), "part 1");
        assert_eq!(issue("1 PT. 2"), "issue 1, part 2");
        assert_eq!(issue("2 Pt l"), "issue 2, part l");
        assert_eq!(issue("8B Pt 2"), "issue 8B, part 2");
        assert_eq!(issue("2-3 Pt 1"), "issue 2-3, part 1");
        assert_eq!(issue("4 Part B"), "issue 4, part B");
        assert_eq!(issue("7S_Part_4"), "issue 7S, part 4");
        assert_eq!(volume("73 ( Pt 11)"), "volume 73, part 11");
        assert_eq!(volume("280 (PT 3)"), "volume 280, part 3");
        assert_eq!(volume("273(Pt 1)"), "volume 273, part 1");
        assert_eq!(volume("67 Suppl 2 Pt 1"), "volume 67, supplement 2, part 1");
        // A letter run is a word whole, not a part of one.
        assert_eq!(issue("Sept"), "issue Sept");
    }

    #[test]
    fn special_issues() {
        assert_eq!(volume("55 Spec No"), "volume 55, special issue");
        assert_eq!(volume("21 spec no"), "volume 21, special issue");
        assert_eq!(volume("16 Spec No 1"), "volume 16, special issue 1");
        assert_eq!(volume("Spec No"), "special issue");
        assert_eq!(volume("Spec No 9"), "special issue 9");
        assert_eq!(issue("1300 SPEC NO"), "issue 1300, special issue");
        assert_eq!(issue("Spec Issue A"), "special issue A");
        assert_eq!(volume("Special Issue 1"), "special issue 1");
        assert_eq!(volume("Hors-série"), "special issue");
        assert_eq!(volume("HORS-SÉRIE"), "special issue");
        // `Spec` alone is not a marker.
        assert_eq!(issue("Spec"), "issue Spec");
    }

    #[test]
    fn series() {
        assert_eq!(volume("s2-XI"), "series 2, volume XI");
        assert_eq!(volume("S5-X"), "series 5, volume X");
        assert!(Label::read("s2-XI", LevelKind::Volume).1.series);
        // Not the counted shape.
        assert_eq!(volume("S1-2"), "volume S1-2");
        assert_eq!(volume("MA2018-01"), "volume MA2018-01");
        // Only a volume's.
        assert_eq!(issue("s2-XI"), "issue s2-XI");
    }

    #[test]
    fn kept() {
        assert_eq!(volume("36 Suppl 2 Incretin"), "kept 36 Suppl 2 Incretin");
        assert_eq!(volume("22(9)Suppl"), "kept 22(9)Suppl");
        assert_eq!(
            issue("Cumulative Annual Pocket Pt"),
            "kept Cumulative Annual Pocket Pt"
        );
    }

    #[test]
    fn placeholders() {
        assert_eq!(volume("&NA;"), "none");
        assert_eq!(volume("Online first"), "none");
        assert_eq!(volume("ONLINE FIRST"), "none");
        assert_eq!(issue("  "), "none");
        let readings = Label::read("&NA;", LevelKind::Volume).1;
        assert_eq!(
            readings.made().collect::<Vec<_>>(),
            ["a placeholder read as no label"]
        );
    }

    #[test]
    fn rendered_for_reports() {
        let render = |s, own| Label::read(s, own).0.map(|l| l.to_string());
        assert_eq!(
            render("20 suppl 2", LevelKind::Volume).as_deref(),
            Some("20 Suppl 2")
        );
        assert_eq!(
            render("5_suppl_2", LevelKind::Issue).as_deref(),
            Some("5 Suppl 2")
        );
        assert_eq!(
            render("s2-XI", LevelKind::Volume).as_deref(),
            Some("Ser 2 XI")
        );
        assert_eq!(render("3-4", LevelKind::Issue).as_deref(), Some("3-4"));
    }

    #[test]
    fn stored_and_read_back() {
        for (s, own) in [
            ("45", LevelKind::Volume),
            ("67 Suppl 2 Pt 1", LevelKind::Volume),
            ("36 Suppl 2 Incretin", LevelKind::Volume),
            ("Suppl", LevelKind::Issue),
        ] {
            let label = Label::read(s, own).0.unwrap();
            let bytes = postcard::to_allocvec(&label).unwrap();
            let back: Label<'_> = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(format!("{back:?}"), format!("{label:?}"));
        }
    }
}
