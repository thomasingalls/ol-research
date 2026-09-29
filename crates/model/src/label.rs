//! A volume's or an issue's label, read (citation fields plan D12, model
//! version 13): text, as the source writes it, with the levels written
//! into it read out. `20 Suppl 2` in a volume is volume `20`, then
//! supplement `2`; `2 Pt 1` in an issue is issue `2`, then part `1`;
//! `Suppl` in an issue is a supplement and no issue; `Volume 17 Issue 1`
//! is volume `17`, then issue `1`. What is not a marker stays as written:
//! combined issues (`3-4`, `28/29`), `2022.1`, `[1977] VR`. 98.8% of
//! PubMed's volumes and 95.4% of its issues are plain digits, and 97.8%
//! and 92.7% of Crossref's journal articles' (`ol-rht-survey-placement-v1`,
//! 2026-09-29); the rest carry what a citation needs.
//!
//! The markers read are those the surveys counted
//! (`ol-rht-survey-placement-v1`, then `ol-rht-survey-crossref-v5` and
//! `ol-rht-survey-baseline-v9` at model 13):
//! - a supplement: `Suppl`, `Supp`, `Sup`, `Supplement`, with a full stop
//!   or without, joined to what is beside it or not (`11suppl`, `3_suppl`,
//!   `Supplement_1`, `5_suppl_2`, `Supplement-1`, `sup1`), and an issue
//!   that is `S` and a number (`S6`, `s73`, `S 01`);
//! - a part: `Pt`, `Part` (`2 Pt 1`, `73 ( Pt 11)`, `PT1`, `3Pt2`,
//!   `Pt. 1`, `4 Part B`, `7S_Part_4`);
//! - a special issue: `Spec No`, `Spec Issue`, `Special Issue`, `Special`,
//!   `Hors-série`, `Numéro spécial`, `Especial`, and `sp`, `spe`, `si`
//!   before a number (`sp1`, `spe1`, `si2`);
//! - a section, which is named: `Laryngol Sect`, `Sect Study Dis Child`,
//!   or lettered before a number: `Sect B 1`;
//! - a series before a volume: `s2-XI`, `S5-X`, old journals' volumes
//!   numbered within a series (D13);
//! - the level's own name: `Volume 21`, `Vol. 47`, `Tome 91`, `v.51`,
//!   `Issue 1`, and a number sign, `n° 93`, `No. 6`, `№2`, which names
//!   the field's own level, or the issue after a volume (`Vol. 19, N° 1-2`
//!   is volume `19`, issue `1-2`). A volume the source numbers like an
//!   issue stays a volume (the user, 2026-09-29).
//!
//! Each marker's label is what follows it, up to the next marker: `1E` of
//! `17 Suppl 1E`. What reading `S6` or `s2-XI` supposes is returned beside
//! the label ([`Readings`]), to be counted as a page field's readings are.
//! A marker in a shape this reading does not know is kept as written
//! ([`Label::Kept`]) and counted by shape: `36 Suppl 2 Incretin`,
//! `22(9)Suppl`. A placeholder (`&NA;`, `null`) reads as no label. Words
//! saying the work is not in an issue yet (`ahead-of-print`, IEEE's `PP`,
//! `Online first`) are a different kind (the user, 2026-09-29): kept as
//! written ([`Label::Pending`]), a statement about the work, not a label.

use std::fmt;

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// A volume or an issue field, read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Label<'a> {
    /// No marker in it: the level's label as written, less the space
    /// around it (`45`, `3-4`, `2022.1`).
    Plain(&'a str),
    /// The levels written in it, outermost first, each with its label.
    Read(#[serde(borrow)] SmallVec<[Level<'a>; 2]>),
    /// Words saying the work was not in a volume or an issue yet when the
    /// record was written, as written: `ahead-of-print`, IEEE's early
    /// access `PP` (and its issue `99`), `Online first`, `Forthcoming`.
    /// Often stale: NLM's records of IEEE articles printed years since
    /// still say `PP` (PMID 21359137, of 2011).
    Pending(&'a str),
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
    // above either is `Series`. A journal has sections too: the
    // Proceedings of the Royal Society of Medicine's `Laryngol Sect`.
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

    /// A volume or an issue: the levels a field is for, whose name alone
    /// is not a level (`N°` with no number after it).
    const fn is_numbered(self) -> bool {
        matches!(self, Self::Volume | Self::Issue)
    }
}

/// What reading a label supposed beyond the markers written in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)] // independent facts
pub struct Readings {
    /// A placeholder was read as no label.
    pub placeholder: bool,
    /// Words saying the work awaits its issue were kept as such.
    pub pending: bool,
    /// An issue that is `S` and a number was read as a supplement.
    pub s_number: bool,
    /// A volume written after a series (`s2-XI`) was read as two levels.
    pub series: bool,
    /// A level's own name (`Volume`, `n°`) was read before its label.
    pub named: bool,
}

impl Readings {
    /// Each reading made, in words, for a report.
    pub fn made(self) -> impl Iterator<Item = &'static str> {
        [
            (self.placeholder, "a placeholder read as no label"),
            (self.pending, "words saying the work awaits its issue"),
            (self.s_number, "S and a number read as a supplement"),
            (self.series, "a series read before the volume"),
            (self.named, "a level's name read before its label"),
        ]
        .into_iter()
        .filter_map(|(made, what)| made.then_some(what))
    }
}

/// Words a source writes in a volume or an issue for a work not in one
/// yet, in any case: 411 PubMed volumes `ahead-of-print`; 3,017 `PP`,
/// every one on the laptop an IEEE journal's article `aheadofprint`;
/// 18,307 Crossref volumes `Online first`.
const PENDING: [&str; 5] = [
    "ahead-of-print",
    "ahead of print",
    "PP",
    "Online first",
    "Forthcoming",
];

/// Whether a placement field, less the space around it, says the work is
/// not in an issue yet.
pub(crate) fn is_pending(text: &str) -> bool {
    PENDING.iter().any(|p| p.eq_ignore_ascii_case(text))
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
        if is_pending(text) {
            readings.pending = true;
            return (Some(Self::Pending(text)), readings);
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
        let mut marks = markers(text);
        // A field that ends in its own level's name names itself: `Web
        // Server issue`, NAR's.
        if marks
            .last()
            .is_some_and(|m| !matches!(m.mark, Mark::Level(k) if !k.is_numbered()))
            && between(&text[marks[marks.len() - 1].end..]).is_none()
        {
            marks.pop();
        }
        if marks.is_empty() {
            return (Some(Self::Plain(text)), readings);
        }
        match levels(text, own, &marks, &mut readings) {
            Some(levels) => (Some(Self::Read(levels)), readings),
            None => (Some(Self::Kept(text)), Readings::default()),
        }
    }

    /// Reads an issue beside the record's volume: IEEE's issue `99`
    /// beside its early-access volume `PP` awaits its issue as the volume
    /// does (PMIDs 21359137, 26167111).
    #[must_use]
    pub fn read_issue(field: &'a str, volume: Option<&Label<'_>>) -> (Option<Self>, Readings) {
        let text = field.trim();
        if text == "99" && matches!(volume, Some(Label::Pending(_))) {
            let readings = Readings {
                pending: true,
                ..Readings::default()
            };
            return (Some(Self::Pending(text)), readings);
        }
        Self::read(field, LevelKind::Issue)
    }

    /// A supplement written before a page field's pages, as NLM writes
    /// some: `suppl 4:176-80` is supplement `4`, then pages `176-80`. The
    /// level, and the text of the pages.
    #[must_use]
    pub fn before_pages(field: &'a str) -> Option<(Level<'a>, &'a str)> {
        let (prefix, pages) = field.split_once(':')?;
        match Self::read(prefix, LevelKind::Issue).0? {
            Self::Read(levels) if levels.len() == 1 && levels[0].kind == LevelKind::Supplement => {
                Some((levels[0], pages))
            }
            _ => None,
        }
    }

    /// The levels it names, outermost first: a plain or a kept label names
    /// `own`, the level the field is for; a pending one names none.
    pub fn levels(&self, own: LevelKind) -> impl Iterator<Item = Level<'a>> + '_ {
        let (whole, read) = match self {
            Self::Plain(text) | Self::Kept(text) => (Some(*text), &[][..]),
            Self::Read(levels) => (None, &levels[..]),
            Self::Pending(_) => (None, &[][..]),
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
            Self::Plain(text) | Self::Kept(text) | Self::Pending(text) => f.write_str(text),
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

/// What a marker opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Level(LevelKind),
    /// A number sign (`n°`, `No.`, `№`, `Number`): the field's own level,
    /// or the issue after a volume.
    Number,
}

/// A marker found in a label: where it is, and what it opens.
#[derive(Debug, Clone, Copy)]
struct Marker {
    start: usize,
    end: usize,
    mark: Mark,
}

/// Two-word markers, tried first: the first word, and the words that may
/// follow it.
const PAIRS: [(&str, &[&str]); 5] = [
    ("spec", &["no", "issue"]),
    ("special", &["issue"]),
    ("hors", &["série", "serie"]),
    ("numéro", &["spécial", "special"]),
    ("numero", &["spécial", "special"]),
];

/// The markers in a label, in the order written.
fn markers(text: &str) -> SmallVec<[Marker; 2]> {
    let words = words(text);
    let mut out = SmallVec::new();
    let mut i = 0;
    while i < words.len() {
        let (start, end) = words[i];
        let word = &text[start..end];
        let after = &text[end..];
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
        let pair = PAIRS
            .iter()
            .any(|(first, then)| is(word, &[first]) && next.is_some_and(|n| is(n, then)));
        let digit_follows = after
            .trim_start_matches(['.', ' '])
            .starts_with(|c: char| c.is_ascii_digit());
        let (mark, took, end) = if pair {
            (
                Some(Mark::Level(LevelKind::SpecialIssue)),
                2,
                words[i + 1].1,
            )
        } else if is(word, &["suppl", "supp", "sup", "supplement"]) {
            (Some(Mark::Level(LevelKind::Supplement)), 1, end)
        } else if is(word, &["pt", "part"]) {
            (Some(Mark::Level(LevelKind::Part)), 1, end)
        } else if is(word, &["special", "especial"])
            || (is(word, &["sp", "spe", "si"]) && digit_follows)
        {
            (Some(Mark::Level(LevelKind::SpecialIssue)), 1, end)
        } else if is(word, &["sect", "section"]) {
            (Some(Mark::Level(LevelKind::Section)), 1, end)
        } else if is(word, &["vol", "volume", "tome"])
            || (is(word, &["v", "t"]) && after.starts_with('.') && digit_follows)
        {
            (Some(Mark::Level(LevelKind::Volume)), 1, end)
        } else if is(word, &["issue"]) {
            (Some(Mark::Level(LevelKind::Issue)), 1, end)
        } else if word == "№" || is(word, &["nº", "numéro", "numero", "number", "nr"]) {
            (Some(Mark::Number), 1, end)
        } else if is(word, &["n"]) && after.starts_with(['°', 'º']) {
            let sign = after.chars().next().map_or(0, char::len_utf8);
            (Some(Mark::Number), 1, end + sign)
        } else if is(word, &["no"]) && (after.starts_with('.') || digit_follows) {
            (Some(Mark::Number), 1, end)
        } else {
            (None, 1, end)
        };
        if let Some(mark) = mark {
            out.push(Marker { start, end, mark });
        }
        i += took;
    }
    out
}

/// Where each run of letters starts and ends; `№` is a word of its own.
fn words(text: &str) -> SmallVec<[(usize, usize); 4]> {
    let mut out = SmallVec::new();
    let mut start = None;
    for (at, c) in text.char_indices() {
        let letter = c.is_alphabetic();
        if let Some(s) = start
            && !letter
        {
            out.push((s, at));
            start = None;
        }
        if c == '№' {
            out.push((at, at + c.len_utf8()));
        } else if letter && start.is_none() {
            start = Some(at);
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
fn levels<'a>(
    text: &'a str,
    own: LevelKind,
    marks: &[Marker],
    readings: &mut Readings,
) -> Option<SmallVec<[Level<'a>; 2]>> {
    if marks
        .iter()
        .any(|m| m.mark == Mark::Level(LevelKind::Section))
    {
        return section(text, own, marks);
    }
    let mut out: SmallVec<[Level<'a>; 2]> = SmallVec::new();
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
        let kind = match mark.mark {
            Mark::Level(kind) => kind,
            Mark::Number if out.iter().any(|l| l.kind == LevelKind::Volume) => LevelKind::Issue,
            Mark::Number => own,
        };
        if mark.mark == Mark::Number || kind.is_numbered() {
            readings.named = true;
        }
        // A level's name with no label names the next level: `N°
        // Hors-série` is a special issue, not a volume `N°`.
        if label.is_some() || !kind.is_numbered() {
            out.push(Level { kind, label });
        }
    }
    (!out.is_empty()).then_some(out)
}

/// A label with a section in it. A section is named, before its marker or
/// after it (`Laryngol Sect`, `Sect Study Dis Child`), or lettered before
/// the field's own number (`Sect B 1`, `Sect. C 27`). Anything else is not
/// read: `Section 38-1322 as amended 1977` is a statute's.
fn section<'a>(
    text: &'a str,
    own: LevelKind,
    marks: &[Marker],
) -> Option<SmallVec<[Level<'a>; 2]>> {
    let [mark] = marks else {
        return None;
    };
    let named = |s: &str| !s.bytes().any(|b| b.is_ascii_digit());
    let level = |kind, label| Level {
        kind,
        label: Some(label),
    };
    match (between(&text[..mark.start]), between(&text[mark.end..])) {
        (Some(name), None) | (None, Some(name)) if named(name) => {
            Some(SmallVec::from_elem(level(LevelKind::Section, name), 1))
        }
        (None, Some(rest)) => {
            let (name, number) = rest.split_once(' ')?;
            let number = number.trim();
            (named(name) && !name.is_empty() && is_one(number))
                .then(|| SmallVec::from_buf([level(LevelKind::Section, name), level(own, number)]))
        }
        _ => None,
    }
}

/// The text between two markers, less what joins it to them (space, `_`,
/// `-`, `.`, `,`, `:`, brackets); `None` where nothing is left.
fn between(piece: &str) -> Option<&str> {
    let piece = piece.trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, '_' | '-' | '.' | ',' | ':' | '(' | ')')
    });
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

    /// The levels read, as `kind label`, or `kept`, `pending` or `none`.
    fn read(s: &str, own: LevelKind) -> String {
        render(Label::read(s, own).0, own)
    }

    fn render(label: Option<Label<'_>>, own: LevelKind) -> String {
        match label {
            None => "none".into(),
            Some(Label::Kept(text)) => format!("kept {text}"),
            Some(Label::Pending(text)) => format!("pending {text}"),
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
        assert_eq!(volume("2022.1"), "volume 2022.1");
        assert_eq!(volume("[1977] VR"), "volume [1977] VR");
        assert_eq!(issue("Second Half"), "issue Second Half");
        assert_eq!(issue("1A"), "issue 1A");
        assert_eq!(issue("SP"), "issue SP");
        assert_eq!(volume("MA2025-02"), "volume MA2025-02");
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
        // Taylor & Francis's, and NLM's shorter spelling.
        assert_eq!(issue("sup1"), "supplement 1");
        assert_eq!(issue("Sup 3"), "supplement 3");
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
        // JJAP's part, above its issue.
        assert_eq!(issue("Part 2, No. 2B"), "part 2, issue 2B");
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
        assert_eq!(volume("Numéro spécial"), "special issue");
        assert_eq!(issue("Special_3"), "special issue 3");
        assert_eq!(issue("1(Special)"), "issue 1, special issue");
        assert_eq!(issue("Especial (3)"), "special issue 3");
        assert_eq!(issue("sp1"), "special issue 1");
        assert_eq!(issue("spe1"), "special issue 1");
        assert_eq!(issue("si2"), "special issue 2");
        assert_eq!(issue("Special Issue: ICCMA"), "special issue ICCMA");
        // `Spec` alone is not a marker, nor `sp` without a number.
        assert_eq!(issue("Spec"), "issue Spec");
        assert_eq!(issue("SP"), "issue SP");
    }

    #[test]
    fn sections() {
        assert_eq!(issue("Laryngol Sect"), "section Laryngol");
        assert_eq!(issue("Obstet Gynaecol Sect"), "section Obstet Gynaecol");
        assert_eq!(issue("Sect Study Dis Child"), "section Study Dis Child");
        assert_eq!(issue("Clin_Sect"), "section Clin");
        assert_eq!(issue("Sect B 1"), "section B, issue 1");
        assert_eq!(issue("Sect. C 27"), "section C, issue 27");
        // A statute's section, and a section with another marker.
        assert_eq!(
            volume("Section 38-1322 as amended 1977"),
            "kept Section 38-1322 as amended 1977"
        );
        assert_eq!(volume("Section 76.7 Pt 3"), "kept Section 76.7 Pt 3");
    }

    #[test]
    fn a_levels_own_name() {
        assert_eq!(volume("Volume 21"), "volume 21");
        assert_eq!(volume("VOLUME 27"), "volume 27");
        assert_eq!(volume("Vol. 47"), "volume 47");
        assert_eq!(volume("Vol. IX"), "volume IX");
        assert_eq!(volume("Tome 91"), "volume 91");
        assert_eq!(volume("v.51"), "volume 51");
        assert!(Label::read("Vol. 47", LevelKind::Volume).1.named);
        // A number sign names the field's own level.
        assert_eq!(volume("n° 229"), "volume 229");
        assert_eq!(volume("n°141"), "volume 141");
        assert_eq!(volume("N° 193-196"), "volume 193-196");
        assert_eq!(volume("no. 6"), "volume 6");
        assert_eq!(issue("No 30"), "issue 30");
        assert_eq!(issue("№2"), "issue 2");
        assert_eq!(issue("Nº 4"), "issue 4");
        // Or the issue, after a volume.
        assert_eq!(volume("Vol. 19, N° 1-2"), "volume 19, issue 1-2");
        assert_eq!(volume("T.43 N° 2"), "volume 43, issue 2");
        assert_eq!(volume("12 n° 3"), "volume 12, issue 3");
        assert_eq!(issue("Volume 17 Issue 1"), "volume 17, issue 1");
        assert_eq!(issue("Volume 32 - Number 4"), "volume 32, issue 4");
        assert_eq!(volume("Volume 22, Issue 1"), "volume 22, issue 1");
        // A name with nothing after it names the next level, or the field.
        assert_eq!(volume("N° Hors-série"), "special issue");
        assert_eq!(issue("Web Server issue"), "issue Web Server issue");
        assert_eq!(issue("Database issue"), "issue Database issue");
        // Not names: a lone `V`, `T` or `No`.
        assert_eq!(volume("V"), "volume V");
        assert_eq!(issue("No"), "issue No");
        assert_eq!(issue("ISSUE 1 (MARCH) 2026"), "kept ISSUE 1 (MARCH) 2026");
    }

    #[test]
    fn series() {
        assert_eq!(volume("s2-XI"), "series 2, volume XI");
        assert_eq!(volume("S5-X"), "series 5, volume X");
        assert!(Label::read("s2-XI", LevelKind::Volume).1.series);
        // Not the counted shape.
        assert_eq!(volume("S1-2"), "volume S1-2");
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
        // Kept, it supposes nothing.
        assert_eq!(
            Label::read("Vol. 3 of 4", LevelKind::Volume).1,
            Readings::default()
        );
    }

    #[test]
    fn placeholders() {
        assert_eq!(volume("&NA;"), "none");
        assert_eq!(volume("null"), "none");
        assert_eq!(issue("None"), "none");
        assert_eq!(issue("n/a"), "none");
        assert_eq!(issue("  "), "none");
        let readings = Label::read("&NA;", LevelKind::Volume).1;
        assert_eq!(
            readings.made().collect::<Vec<_>>(),
            ["a placeholder read as no label"]
        );
    }

    #[test]
    fn pending() {
        assert_eq!(volume("ahead-of-print"), "pending ahead-of-print");
        assert_eq!(volume("PP"), "pending PP");
        assert_eq!(volume("Online first"), "pending Online first");
        assert_eq!(volume("ONLINE FIRST"), "pending ONLINE FIRST");
        assert_eq!(issue("Forthcoming"), "pending Forthcoming");
        assert!(Label::read("PP", LevelKind::Volume).1.pending);
        assert_eq!(
            Label::read("PP", LevelKind::Volume)
                .0
                .unwrap()
                .levels(LevelKind::Volume)
                .count(),
            0
        );
        // IEEE's issue 99 waits with its volume, and only then.
        let pp = Label::read("PP", LevelKind::Volume).0;
        let (issue, readings) = Label::read_issue("99", pp.as_ref());
        assert_eq!(render(issue, LevelKind::Issue), "pending 99");
        assert!(readings.pending);
        let twelve = Label::read("12", LevelKind::Volume).0;
        let (issue, _) = Label::read_issue("99", twelve.as_ref());
        assert_eq!(render(issue, LevelKind::Issue), "issue 99");
    }

    #[test]
    fn a_supplement_before_pages() {
        let level = |s| Label::before_pages(s).map(|(l, rest)| (l.kind, l.label, rest));
        assert_eq!(
            level("suppl 4:176-80"),
            Some((LevelKind::Supplement, Some("4"), "176-80"))
        );
        assert_eq!(
            level("suppl:85-7"),
            Some((LevelKind::Supplement, None, "85-7"))
        );
        assert_eq!(
            level("Suppl172:206-9"),
            Some((LevelKind::Supplement, Some("172"), "206-9"))
        );
        assert_eq!(
            level("Suppl:B11-2"),
            Some((LevelKind::Supplement, None, "B11-2"))
        );
        assert_eq!(level("2:25-9"), None);
        assert_eq!(level("117-26"), None);
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
        assert_eq!(render("PP", LevelKind::Volume).as_deref(), Some("PP"));
    }

    #[test]
    fn stored_and_read_back() {
        for (s, own) in [
            ("45", LevelKind::Volume),
            ("67 Suppl 2 Pt 1", LevelKind::Volume),
            ("36 Suppl 2 Incretin", LevelKind::Volume),
            ("Suppl", LevelKind::Issue),
            ("PP", LevelKind::Volume),
        ] {
            let label = Label::read(s, own).0.unwrap();
            let bytes = postcard::to_allocvec(&label).unwrap();
            let back: Label<'_> = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(format!("{back:?}"), format!("{label:?}"));
        }
    }
}
