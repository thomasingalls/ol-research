//! A work's page field, read (citation fields plan D8, model version 13):
//! one range or several, in the source's order, each page as the source
//! writes it, and a shortened end written whole. NLM's `123-9` is 123 to
//! 129; over the 12,235,229 works whose PubMed and Crossref records both
//! give a range, expanding PubMed's shortened end gave Crossref's range in
//! 98.8% (`ol-rht-survey-placement-v1`, 2026-09-29), and where it did not,
//! the two disagree on the end page itself.
//!
//! What the reading changed is returned beside it ([`Readings`]), to be
//! counted as a DOI's readings are. A shape it does not read whole is kept
//! as written, beside its first page where that is certain
//! ([`Pages::Kept`]), as a `MedlineDate` is. A placeholder (`UNKNOWN`,
//! `None`, `n/a`) reads as no pages, and so do words saying the work awaits
//! its issue (`Online first`) and a page count (`26 pp.`). A page the
//! source names rather than numbers (`IFC`, an inside front cover) is
//! kept as a named page ([`Pages::Named`]): it has no number to compare,
//! but it is where the work sits (the user, 2026-09-29).
//!
//! The shapes read are those the survey counted (plan D8):
//! - a page or a range: `1033`, `117-26`, `259-264`;
//! - letters before or after a page number: `S202-7`, `274P-275P`, `E164`,
//!   and the letters carried to a shortened end (`S202-7` ends at `S207`);
//! - a sub-page: `163-171.e2`, `424.e21-30`;
//! - roman numerals: `III-VI`;
//! - lists, some labelled: `526-7, 530`, `488-94; discussion 95-6`,
//!   `387; author reply 387-8`, `519-24 contd`, `125-30, passim`;
//! - an article number written as a range of itself: `21-00323-21-00323`;
//! - a range of hyphenated numbers: `A-1-A-5`, `S4-19-S4-20`;
//! - `???` between two pages, an en dash an encoding lost:
//!   `1067???1074`;
//! - an item's number on its first page (`36.1-37`, the first item on page
//!   36 to page 37; `266_1-266`), and a page and those after it (`1098+`,
//!   read as its first, the `+` its note);
//! - the record's issue written before its pages (`2_263-2_285` in issue
//!   `2`), which [`Pages::read_in`] drops, and a supplement written before
//!   them (`suppl 4:176-80`), which [`Pages::read_placed`] reads out as a
//!   level.
//!
//! These last came from the whole files' surveys at model 13
//! (`ol-rht-survey-crossref-v5`, `ol-rht-survey-baseline-v9`, 2026-09-29).

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::label::{Label, Level};

/// A page field, read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pages<'a> {
    /// Read whole: one range or more, in the source's order.
    Read(#[serde(borrow)] SmallVec<[Range<'a>; 1]>),
    /// Not read whole: the first page, where the field begins with one,
    /// and the field as written.
    Kept {
        /// The first page, as written.
        #[serde(borrow)]
        first: Option<&'a str>,
        /// The field as written.
        raw: &'a str,
        /// Why it was not read whole.
        error: Error,
    },
    /// A page the source names, not numbers, as written: Elsevier's `IFC`,
    /// `OBC` (inside front cover, outside back cover), `Cover`, `Toc`.
    Named(&'a str),
}

/// One range of pages, or one page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range<'a> {
    /// Words the source writes before the pages: `discussion`,
    /// `author reply`, `quiz`.
    #[serde(borrow)]
    pub label: Option<&'a str>,
    /// The first page, as written.
    pub first: &'a str,
    /// The last page, written whole: `123-9` ends at `129`. `None` for one
    /// page.
    #[serde(borrow)]
    pub last: Option<Cow<'a, str>>,
    /// Words the source writes after: `contd`, `concl`, `passim`.
    #[serde(borrow)]
    pub note: Option<&'a str>,
}

/// Why a page field was kept as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Error {
    /// A shortened end that, written whole, comes before its start:
    /// `269-38`, `2109-020`. A typo in every example the survey showed
    /// (`209-3` where Crossref has `290-293`).
    ExpandsBefore,
    /// A whole end before its start: `293-204`.
    Backwards,
    /// A shape this reading does not know.
    Shape,
}

impl Error {
    /// The error, in words, for a report.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExpandsBefore => "a shortened end that expands to before its start",
            Self::Backwards => "an end before its start",
            Self::Shape => "a shape not read",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What reading a page field changed from what its source wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)] // independent facts
pub struct Readings {
    /// A shortened end was written whole.
    pub shortened: bool,
    /// An article number written as a range of itself was read as one.
    pub doubled: bool,
    /// `???` between two pages was read as a dash.
    pub encoding: bool,
    /// A dash other than `-` between two pages was read as `-`.
    pub dash: bool,
    /// A placeholder was read as no pages.
    pub placeholder: bool,
    /// Words saying the work awaits its issue were read as no pages.
    pub pending: bool,
    /// A page count was read as no pages: `26 pp.`.
    pub page_count: bool,
    /// An item's number on its first page was read: `36.1-37`.
    pub item: bool,
    /// A page and those after it were read as the first: `1098+`.
    pub and_following: bool,
    /// The record's own issue, written before the pages, was dropped.
    pub issue_prefix: bool,
    /// A supplement written before the pages was read as a level.
    pub supplement: bool,
}

impl Readings {
    /// Each reading made, in words, for a report.
    pub fn made(self) -> impl Iterator<Item = &'static str> {
        [
            (self.shortened, "a shortened end written whole"),
            (self.doubled, "an article number written twice read as one"),
            (self.encoding, "??? between pages read as a dash"),
            (self.dash, "a dash read as a hyphen"),
            (self.placeholder, "a placeholder read as no pages"),
            (
                self.pending,
                "words saying the work awaits its issue read as no pages",
            ),
            (self.page_count, "a page count read as no pages"),
            (self.item, "an item's number on its first page read"),
            (
                self.and_following,
                "a page and those after it read as its first",
            ),
            (
                self.issue_prefix,
                "the record's issue before its pages dropped",
            ),
            (
                self.supplement,
                "a supplement before the pages read as a level",
            ),
        ]
        .into_iter()
        .filter_map(|(made, what)| made.then_some(what))
    }
}

/// What a source writes in a placement field (pages, a volume, an issue)
/// for a value it does not have: `UNKNOWN` (14,200 PubMed page fields) and
/// `&NA;` (130,255 Crossref volumes), counted by
/// `ol-rht-survey-placement-v1`; `None` (96,735 Crossref page fields,
/// `None-None`), `n/a` and `na`, `null` (50,332 Crossref volumes, 11,926
/// issues), by the surveys at model 13. One list for every such field: a
/// placeholder means the same wherever it is written. `Online first` was
/// here until the user set words saying the work awaits its issue apart
/// (`label::is_pending`, 2026-09-29).
const PLACEHOLDERS: [&str; 6] = ["UNKNOWN", "&NA;", "None", "n/a", "na", "null"];

/// Whether a placement field, less the space around it, is a placeholder,
/// in any case.
pub(crate) fn is_placeholder(text: &str) -> bool {
    PLACEHOLDERS.iter().any(|p| p.eq_ignore_ascii_case(text))
}

/// The separators of a range: a hyphen, and `???`, an en dash an encoding
/// lost.
const LOST_DASH: &str = "???";

impl<'a> Pages<'a> {
    /// Reads a page field. `None` for nothing but space, a placeholder
    /// (`None-None` too), words saying the work awaits its issue, or a page
    /// count.
    #[must_use]
    pub fn read(field: &'a str) -> (Option<Self>, Readings) {
        let mut readings = Readings::default();
        let text = field.trim();
        if text.is_empty() {
            return (None, readings);
        }
        if is_placeholder(text)
            || (pieces(text).nth(1).is_some() && pieces(text).all(|p| is_placeholder(p.trim())))
        {
            readings.placeholder = true;
            return (None, readings);
        }
        if crate::label::is_pending(text) {
            readings.pending = true;
            return (None, readings);
        }
        if is_page_count(text) {
            readings.page_count = true;
            return (None, readings);
        }
        match read_items(text, &mut readings) {
            Ok(ranges) => (Some(Self::Read(ranges)), readings),
            Err(_) if !text.bytes().any(|b| b.is_ascii_digit()) => {
                (Some(Self::Named(text)), Readings::default())
            }
            Err(error) => (
                Some(Self::Kept {
                    first: certain_first(text),
                    raw: field,
                    error,
                }),
                Readings::default(),
            ),
        }
    }

    /// Reads a page field beside the record's issue: where the field
    /// writes that issue before its pages, joined by `_`, as some of
    /// J-STAGE's journals do, it is dropped (`2_263-2_285` in issue `2` is
    /// pages 263 to 285). Only the record's own issue is dropped; the same
    /// `_` elsewhere is an item's number (`266_1-266`).
    #[must_use]
    pub fn read_in(field: &'a str, issue: Option<&str>) -> (Option<Self>, Readings) {
        let text = field.trim();
        if let Some(issue) = issue
            && let Some((first, last)) = issue_prefixed(text, issue.trim())
        {
            let mut readings = Readings {
                issue_prefix: true,
                ..Readings::default()
            };
            let range = match last {
                None if is_page(first) => Ok(Range::one(first)),
                None => Err(Error::Shape),
                Some(last) => range(first, last, &mut readings),
            };
            return match range {
                Ok(range) => (Some(Self::Read(SmallVec::from_elem(range, 1))), readings),
                Err(error) => (
                    Some(Self::Kept {
                        first: is_page(first).then_some(first),
                        raw: field,
                        error,
                    }),
                    Readings::default(),
                ),
            };
        }
        Self::read(field)
    }

    /// Reads a page field beside the record's issue ([`Pages::read_in`]),
    /// a supplement written before its pages read out as a level of its
    /// own: NLM's `suppl 4:176-80` is supplement `4`, then pages 176 to
    /// 180.
    #[must_use]
    pub fn read_placed(
        field: &'a str,
        issue: Option<&str>,
    ) -> (Option<Level<'a>>, Option<Self>, Readings) {
        let (level, text) = match Label::before_pages(field) {
            Some((level, pages)) => (Some(level), pages),
            None => (None, field),
        };
        let (pages, mut readings) = Self::read_in(text, issue);
        readings.supplement = level.is_some();
        (level, pages, readings)
    }

    /// From a first page and a last, given apart, as PubMed's `StartPage`
    /// and `EndPage` are. Kept, the field holds the first page alone as
    /// `raw`: the two are not one text to borrow. No record of the 2026
    /// baseline gives them (0 of 39,994,988, CF0).
    #[must_use]
    pub fn from_parts(first: &'a str, last: Option<&'a str>) -> (Option<Self>, Readings) {
        let mut readings = Readings::default();
        let first = first.trim();
        if first.is_empty() {
            return (None, readings);
        }
        let range = match last.map(str::trim).filter(|l| !l.is_empty()) {
            None => Ok(Range::one(first)),
            Some(last) => range(first, last, &mut readings),
        };
        match range {
            Ok(range) => (Some(Self::Read(SmallVec::from_elem(range, 1))), readings),
            Err(error) => (
                Some(Self::Kept {
                    first: Some(first),
                    raw: first,
                    error,
                }),
                Readings::default(),
            ),
        }
    }

    /// The first page, where there is one certain; a named page has no
    /// number.
    #[must_use]
    pub fn first(&self) -> Option<&'a str> {
        match self {
            Self::Read(ranges) => ranges.first().map(|r| r.first),
            Self::Kept { first, .. } => *first,
            Self::Named(_) => None,
        }
    }
}

impl<'a> Range<'a> {
    const fn one(page: &'a str) -> Self {
        Self {
            label: None,
            first: page,
            last: None,
            note: None,
        }
    }
}

impl fmt::Display for Pages<'_> {
    /// The pages as read: ranges joined by `, `, each `first-last`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(ranges) => {
                for (i, r) in ranges.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    if let Some(label) = r.label {
                        write!(f, "{label} ")?;
                    }
                    f.write_str(r.first)?;
                    if let Some(last) = &r.last {
                        write!(f, "-{last}")?;
                    }
                    if let Some(note) = r.note {
                        write!(f, " {note}")?;
                    }
                }
                Ok(())
            }
            Self::Kept { raw, .. } | Self::Named(raw) => f.write_str(raw),
        }
    }
}

/// Every item of a list, in order: split on `,` and `;`.
fn read_items<'a>(
    text: &'a str,
    readings: &mut Readings,
) -> Result<SmallVec<[Range<'a>; 1]>, Error> {
    let mut ranges: SmallVec<[Range<'a>; 1]> = SmallVec::new();
    for item in text.split([',', ';']) {
        let item = item.trim();
        if item.is_empty() {
            return Err(Error::Shape);
        }
        match read_item(item, readings)? {
            Item::Pages(range) => ranges.push(range),
            // Words alone after a range are a note on it: `125-30, passim`.
            Item::Words(words) => match ranges.last_mut() {
                Some(last) if last.note.is_none() => last.note = Some(words),
                _ => return Err(Error::Shape),
            },
        }
    }
    Ok(ranges)
}

enum Item<'a> {
    Pages(Range<'a>),
    Words(&'a str),
}

/// One item: words, then pages, then words.
fn read_item<'a>(item: &'a str, readings: &mut Readings) -> Result<Item<'a>, Error> {
    let words: Vec<&'a str> = item.split_whitespace().collect();
    let Some(at) = words.iter().position(|w| is_pages(w)) else {
        return if words.iter().all(|w| is_word(w)) {
            Ok(Item::Words(item))
        } else {
            Err(Error::Shape)
        };
    };
    let before = &words[..at];
    let after = &words[at + 1..];
    if !before.iter().chain(after).all(|w| is_word(w)) {
        return Err(Error::Shape);
    }
    let mut range = read_range(words[at], readings)?;
    range.label = span(item, before);
    if let Some(note) = span(item, after) {
        range.note = Some(note);
    }
    Ok(Item::Pages(range))
}

/// The slice of `item` the words cover, from the first's start to the
/// last's end; `None` for no words.
fn span<'a>(item: &'a str, words: &[&'a str]) -> Option<&'a str> {
    let (first, last) = (words.first()?, words.last()?);
    let start = first.as_ptr() as usize - item.as_ptr() as usize;
    let end = last.as_ptr() as usize - item.as_ptr() as usize + last.len();
    Some(&item[start..end])
}

/// One page or one range, written without spaces.
fn read_range<'a>(text: &'a str, readings: &mut Readings) -> Result<Range<'a>, Error> {
    if let Some(range) = item_range(text) {
        readings.item = range.is_ok();
        return range;
    }
    if let Some(page) = and_following(text) {
        readings.and_following = true;
        return Ok(Range {
            note: Some(&text[page.len()..]),
            ..Range::one(page)
        });
    }
    let separators = separators(text);
    match separators.len() {
        0 => Ok(Range::one(text)),
        1 => {
            let (at, len) = separators[0];
            let (left, right) = (&text[..at], &text[at + len..]);
            // No page before the hyphen: `e-88751` is one article number.
            if !is_page(left) {
                return if is_page(text) || is_hyphenated(text) {
                    Ok(Range::one(text))
                } else {
                    Err(Error::Shape)
                };
            }
            note_separator(&text[at..at + len], readings);
            range(left, right, readings)
        }
        n if n % 2 == 1 => {
            // Hyphenated numbers: split at the middle separator.
            let (at, len) = separators[n / 2];
            let (left, right) = (&text[..at], &text[at + len..]);
            note_separator(&text[at..at + len], readings);
            if left == right {
                readings.doubled = true;
                return Ok(Range::one(left));
            }
            match (stem(left), stem(right)) {
                (Some(a), Some(b)) if a == b => Ok(Range {
                    last: Some(Cow::Borrowed(right)),
                    ..Range::one(left)
                }),
                _ => Err(Error::Shape),
            }
        }
        _ => Err(Error::Shape),
    }
}

/// A range from its first page to its last, the last written whole.
fn range<'a>(first: &'a str, last: &'a str, readings: &mut Readings) -> Result<Range<'a>, Error> {
    if !is_page(first) || !is_page(last) {
        return Err(Error::Shape);
    }
    let whole = Range {
        last: Some(Cow::Borrowed(last)),
        ..Range::one(first)
    };
    // An end with letters of its own is written whole: `S175-S179`,
    // `274P-275P`, `163-171.e2`, `III-VI`.
    if !last.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(whole);
    }
    let Some((prefix, digits)) = split_digits(first) else {
        // A first page ending in letters, a last in digits: as written.
        return Ok(whole);
    };
    // Only a prefix that ends in a letter is carried to the end: `S202-7`,
    // `424.e21-30`. `96.1-95` is not read.
    if !prefix.is_empty() && !prefix.ends_with(|c: char| c.is_ascii_alphabetic()) {
        return Err(Error::Shape);
    }
    let end: Cow<'a, str> = if last.len() < digits.len() {
        let filled = format!("{}{last}", &digits[..digits.len() - last.len()]);
        if filled.as_str() < digits {
            return Err(Error::ExpandsBefore);
        }
        readings.shortened = true;
        Cow::Owned(format!("{prefix}{filled}"))
    } else {
        if last.len() == digits.len() && last < digits {
            return Err(Error::Backwards);
        }
        if prefix.is_empty() {
            Cow::Borrowed(last)
        } else {
            readings.shortened = true;
            Cow::Owned(format!("{prefix}{last}"))
        }
    };
    Ok(Range {
        last: Some(end),
        ..Range::one(first)
    })
}

/// Counts a separator that is not `-`: `???`, or another dash.
fn note_separator(separator: &str, readings: &mut Readings) {
    if separator == LOST_DASH {
        readings.encoding = true;
    } else if separator != "-" {
        readings.dash = true;
    }
}

/// An item's number on its first page, and a whole last page: `36.1-37`
/// is the first item on page 36, to page 37. The BMJ's journals number
/// the letters that share a page so (`143.1-146`, and `143.2-143` for the
/// second on page 143, `10.1136/gut.51.2.143-a`), 104,858 Crossref page
/// fields; some of J-STAGE's write it with `_` (`266_1-266`). A last page
/// before the first is not read: `96.1-95`, and NLM's shortened sub-page
/// (`13.1-11`, Current Protocols' 13.1 to 13.11), which this would misread.
fn item_range(text: &str) -> Option<Result<Range<'_>, Error>> {
    let (first, last) = text.split_once('-')?;
    let (page, item) = first.split_once(['.', '_'])?;
    if !(is_number(page) && is_number(item) && is_number(last)) {
        return None;
    }
    Some(if by_value(last, page).is_ge() {
        Ok(Range {
            last: Some(Cow::Borrowed(last)),
            ..Range::one(first)
        })
    } else {
        Err(Error::Backwards)
    })
}

/// A page and those after it: `1098+`, the page.
fn and_following(text: &str) -> Option<&str> {
    text.strip_suffix('+').filter(|page| is_page(page))
}

/// A page count, not a place: `26 pp.`.
fn is_page_count(text: &str) -> bool {
    let mut words = text.split_whitespace();
    matches!(
        (words.next(), words.next(), words.next()),
        (Some(n), Some(unit), None)
            if is_number(n) && ["pp", "pp.", "p", "p."].iter().any(|u| u.eq_ignore_ascii_case(unit))
    )
}

/// The record's issue written before a field's pages, joined by `_`: the
/// first page and the last, less it. `None` unless the field is so
/// written and the issue is the record's own.
fn issue_prefixed<'a>(text: &'a str, issue: &str) -> Option<(&'a str, Option<&'a str>)> {
    let strip = |s: &'a str| {
        s.split_once('_')
            .filter(|(before, page)| {
                !page.is_empty() && is_number(before) && by_value(before, issue).is_eq()
            })
            .map(|(_, page)| page)
    };
    match text.split_once('-') {
        None => Some((strip(text)?, None)),
        Some((first, last)) => Some((strip(first)?, Some(strip(last).unwrap_or(last)))),
    }
}

/// Digits and nothing else.
fn is_number(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Two numbers' order by value, leading zeros aside; a text that is not
/// a number is compared as written.
fn by_value(a: &str, b: &str) -> std::cmp::Ordering {
    let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// A range's dash: `-`, or any other Unicode dash or the minus sign, as a
/// DOI's are read (`708–718`).
fn is_dash(c: char) -> bool {
    c == '-' || crate::doi::DASHES.contains(&c)
}

/// Where each separator of a range starts, and its length: a dash, or
/// `???`.
fn separators(text: &str) -> SmallVec<[(usize, usize); 3]> {
    let mut out = SmallVec::new();
    let mut i = 0;
    while i < text.len() {
        if text[i..].starts_with(LOST_DASH) {
            out.push((i, LOST_DASH.len()));
            i += LOST_DASH.len();
            continue;
        }
        let c = text[i..].chars().next().unwrap_or('\0');
        if is_dash(c) {
            out.push((i, c.len_utf8()));
        }
        i += c.len_utf8().max(1);
    }
    out
}

/// The text between separators.
fn pieces(text: &str) -> impl Iterator<Item = &str> {
    let separators = separators(text);
    let mut starts = SmallVec::<[usize; 4]>::new();
    let mut ends = SmallVec::<[usize; 4]>::new();
    starts.push(0);
    for &(at, len) in &separators {
        ends.push(at);
        starts.push(at + len);
    }
    ends.push(text.len());
    starts.into_iter().zip(ends).map(move |(s, e)| &text[s..e])
}

/// A page: letters, digits and dots, with a digit in it, or a roman
/// numeral.
fn is_page(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.')
        && (s.bytes().any(|b| b.is_ascii_digit()) || is_roman(s))
}

/// Pages written without spaces: one page, or pages joined by dashes or
/// `???`.
fn is_pages(s: &str) -> bool {
    is_page(s)
        || item_range(s).is_some()
        || and_following(s).is_some()
        || (pieces(s)
            .all(|p| p.is_empty() || p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.'))
            && s.bytes().any(|b| b.is_ascii_digit()))
        || pieces(s).all(is_roman)
}

/// An article number with a hyphen after its letters: `e-88751`.
fn is_hyphenated(s: &str) -> bool {
    let mut parts = s.split('-');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(a), Some(b), None) if !a.is_empty() && a.bytes().all(|c| c.is_ascii_alphabetic()) && is_page(b)
    )
}

/// A word, not a page: letters only, and not a roman numeral.
fn is_word(s: &str) -> bool {
    s.chars().all(char::is_alphabetic) && !is_roman(s)
}

pub(crate) fn is_roman(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b"ivxlcdmIVXLCDM".contains(&b))
}

/// A page's letters before its last run of digits, and the digits; `None`
/// when it does not end in a digit.
fn split_digits(page: &str) -> Option<(&str, &str)> {
    let digits = page.len() - page.bytes().rev().take_while(u8::is_ascii_digit).count();
    (digits < page.len()).then(|| page.split_at(digits))
}

/// A hyphenated number's stem: all before its last hyphen.
fn stem(s: &str) -> Option<&str> {
    s.rfind('-').map(|at| &s[..=at])
}

/// The first page of a field not read whole, where it begins with one.
fn certain_first(text: &str) -> Option<&str> {
    let first = text.split([',', ';', ' ']).next()?;
    let first = pieces(first).next()?;
    is_page(first).then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ranges read, as `label first-last note`.
    fn read(s: &str) -> Vec<String> {
        match Pages::read(s).0 {
            Some(Pages::Read(ranges)) => ranges
                .iter()
                .map(|r| {
                    let mut out = String::new();
                    if let Some(l) = r.label {
                        out.push_str(l);
                        out.push(' ');
                    }
                    out.push_str(r.first);
                    if let Some(last) = &r.last {
                        out.push('-');
                        out.push_str(last);
                    }
                    if let Some(n) = r.note {
                        out.push(' ');
                        out.push_str(n);
                    }
                    out
                })
                .collect(),
            other => panic!("{s:?} not read: {other:?}"),
        }
    }

    fn kept(s: &str) -> (Option<&str>, Error) {
        match Pages::read(s).0 {
            Some(Pages::Kept { first, error, .. }) => (first, error),
            other => panic!("{s:?} read: {other:?}"),
        }
    }

    #[test]
    fn plain_pages_and_ranges() {
        assert_eq!(read("1033"), ["1033"]);
        assert_eq!(read("259-264"), ["259-264"]);
        assert_eq!(read(" 1-10 "), ["1-10"]);
        assert_eq!(read("98-102"), ["98-102"]);
        assert_eq!(read("1365-1365"), ["1365-1365"]);
        assert_eq!(read("3331024251409706"), ["3331024251409706"]);
    }

    #[test]
    fn a_shortened_end_is_written_whole() {
        assert_eq!(read("117-26"), ["117-126"]);
        assert_eq!(read("1173-9"), ["1173-1179"]);
        assert_eq!(read("1299-301"), ["1299-1301"]);
        assert!(Pages::read("117-26").1.shortened);
        assert!(!Pages::read("259-264").1.shortened);
    }

    #[test]
    fn letters_on_pages() {
        assert_eq!(read("S202-7"), ["S202-S207"]);
        assert_eq!(read("A205-6"), ["A205-A206"]);
        assert_eq!(read("C1-3"), ["C1-C3"]);
        assert_eq!(read("S175-S179"), ["S175-S179"]);
        assert_eq!(read("274P-275P"), ["274P-275P"]);
        assert_eq!(read("e201-e204"), ["e201-e204"]);
        assert_eq!(read("E164"), ["E164"]);
        assert_eq!(read("e0233525"), ["e0233525"]);
        assert_eq!(read("290P"), ["290P"]);
        assert_eq!(read("1A-30A"), ["1A-30A"]);
    }

    #[test]
    fn sub_pages() {
        assert_eq!(read("163-171.e2"), ["163-171.e2"]);
        assert_eq!(read("778.e1-778.e6"), ["778.e1-778.e6"]);
        assert_eq!(read("424.e21-30"), ["424.e21-424.e30"]);
        assert_eq!(read("424.e1-10"), ["424.e1-424.e10"]);
        assert_eq!(read("2.29-2.32"), ["2.29-2.32"]);
        assert_eq!(read("1473.2"), ["1473.2"]);
    }

    #[test]
    fn roman_numerals() {
        assert_eq!(read("III-VI"), ["III-VI"]);
        assert_eq!(read("xii-xiii"), ["xii-xiii"]);
        assert_eq!(read("ix"), ["ix"]);
        assert_eq!(read("451-2, x"), ["451-452", "x"]);
    }

    #[test]
    fn lists_labels_and_notes() {
        assert_eq!(read("526-7, 530"), ["526-527", "530"]);
        assert_eq!(read("15-7, 22-4"), ["15-17", "22-24"]);
        assert_eq!(
            read("488-94; discussion 95-6"),
            ["488-494", "discussion 95-96"]
        );
        assert_eq!(
            read("387; author reply 387-8"),
            ["387", "author reply 387-388"]
        );
        assert_eq!(read("43-62; quiz 62-3"), ["43-62", "quiz 62-63"]);
        assert_eq!(read("519-24 contd"), ["519-524 contd"]);
        assert_eq!(read("125-30, passim"), ["125-130 passim"]);
        assert_eq!(
            read("E164; author reply E165"),
            ["E164", "author reply E165"]
        );
    }

    #[test]
    fn hyphenated_numbers() {
        assert_eq!(read("21-00323-21-00323"), ["21-00323"]);
        assert!(Pages::read("21-00323-21-00323").1.doubled);
        assert_eq!(read("A-1-A-5"), ["A-1-A-5"]);
        assert_eq!(read("S4-19-S4-20"), ["S4-19-S4-20"]);
        assert_eq!(read("E21-004-1-E21-004-11"), ["E21-004-1-E21-004-11"]);
        assert_eq!(read("e-88751"), ["e-88751"]);
    }

    #[test]
    fn a_lost_dash() {
        assert_eq!(read("1067???1074"), ["1067-1074"]);
        assert!(Pages::read("1067???1074").1.encoding);
        assert_eq!(read("1???3"), ["1-3"]);
    }

    #[test]
    fn other_dashes() {
        // PMID 30934041 writes an en dash.
        assert_eq!(read("708\u{2013}718"), ["708-718"]);
        assert!(Pages::read("708\u{2013}718").1.dash);
        assert_eq!(read("117\u{2014}26"), ["117-126"]);
        assert_eq!(read("5\u{2212}9"), ["5-9"]);
        assert!(!Pages::read("5-9").1.dash);
    }

    #[test]
    fn placeholders_and_nothing() {
        assert_eq!(Pages::read("UNKNOWN").0, None);
        assert!(Pages::read("UNKNOWN").1.placeholder);
        assert_eq!(Pages::read("unknown").0, None);
        assert_eq!(Pages::read("  ").0, None);
        for none in ["None", "none", "n/a", "na", "null", "None-None", "n/a-n/a"] {
            let (pages, readings) = Pages::read(none);
            assert_eq!(pages, None, "{none}");
            assert!(readings.placeholder, "{none}");
        }
        // Words saying the work awaits its issue, apart.
        let (pages, readings) = Pages::read("Online first");
        assert_eq!(pages, None);
        assert!(readings.pending && !readings.placeholder);
        // A page count is not a place.
        for count in ["26 pp.", "8 pp", "12 p."] {
            let (pages, readings) = Pages::read(count);
            assert_eq!(pages, None, "{count}");
            assert!(readings.page_count, "{count}");
        }
    }

    #[test]
    fn named_pages() {
        for name in [
            "IFC",
            "OBC",
            "Cover",
            "Toc",
            "inside front cover",
            "see text",
        ] {
            assert_eq!(Pages::read(name).0, Some(Pages::Named(name)), "{name}");
        }
        assert_eq!(Pages::read("IFC").0.and_then(|p| p.first()), None);
        // A roman page is numbered.
        assert_eq!(read("xii"), ["xii"]);
    }

    #[test]
    fn an_items_number_on_its_page() {
        assert_eq!(read("36.1-37"), ["36.1-37"]);
        assert!(Pages::read("36.1-37").1.item);
        assert_eq!(read("143.2-143"), ["143.2-143"]);
        assert_eq!(read("1074.1-1076"), ["1074.1-1076"]);
        assert_eq!(read("266_1-266"), ["266_1-266"]);
        // Sub-pages written whole are not items.
        assert_eq!(read("2.29-2.32"), ["2.29-2.32"]);
        assert!(!Pages::read("2.29-2.32").1.item);
    }

    #[test]
    fn a_page_and_those_after_it() {
        assert_eq!(read("1098+"), ["1098 +"]);
        assert!(Pages::read("1098+").1.and_following);
        assert_eq!(Pages::read("1098+").0.and_then(|p| p.first()), Some("1098"));
    }

    #[test]
    fn the_records_issue_before_its_pages() {
        let read_in = |s, issue| Pages::read_in(s, issue).0.map(|p| p.to_string());
        assert_eq!(
            read_in("2_263-2_285", Some("2")).as_deref(),
            Some("263-285")
        );
        assert_eq!(read_in("4_2-11", Some("4")).as_deref(), Some("2-11"));
        assert_eq!(read_in("4_2-11", Some("04")).as_deref(), Some("2-11"));
        assert_eq!(read_in("1_36", Some("1")).as_deref(), Some("36"));
        assert!(Pages::read_in("4_2-11", Some("4")).1.issue_prefix);
        // Not the record's issue: not dropped.
        assert!(matches!(
            Pages::read_in("2_263-2_285", Some("3")).0,
            Some(Pages::Kept { .. })
        ));
        assert!(matches!(
            Pages::read_in("2_263-2_285", None).0,
            Some(Pages::Kept { .. })
        ));
        // An item's number, where the issue is not the page's stem.
        assert_eq!(
            read_in("266_1-266", Some("2")).as_deref(),
            Some("266_1-266")
        );
    }

    #[test]
    fn a_supplement_before_the_pages() {
        use crate::label::LevelKind;
        let (level, pages, readings) = Pages::read_placed("suppl 4:176-80", None);
        let level = level.unwrap();
        assert_eq!(
            (level.kind, level.label),
            (LevelKind::Supplement, Some("4"))
        );
        assert_eq!(pages.map(|p| p.to_string()).as_deref(), Some("176-180"));
        assert!(readings.supplement && readings.shortened);
        let (level, pages, _) = Pages::read_placed("Suppl 2:1092+", None);
        assert_eq!(level.map(|l| l.label), Some(Some("2")));
        assert_eq!(pages.map(|p| p.to_string()).as_deref(), Some("1092 +"));
        let (level, pages, readings) = Pages::read_placed("117-26", None);
        assert!(level.is_none() && !readings.supplement);
        assert_eq!(pages.map(|p| p.to_string()).as_deref(), Some("117-126"));
    }

    #[test]
    fn what_is_kept() {
        assert_eq!(kept("269-38"), (Some("269"), Error::ExpandsBefore));
        assert_eq!(kept("2109-020"), (Some("2109"), Error::ExpandsBefore));
        assert_eq!(kept("293-204"), (Some("293"), Error::Backwards));
        assert_eq!(kept("96.1-95"), (Some("96.1"), Error::Backwards));
        // NLM's shortened sub-page, which the item reading would misread.
        assert_eq!(kept("13.1-11"), (Some("13.1"), Error::Backwards));
        assert_eq!(kept("949-a-950"), (Some("949"), Error::Shape));
        assert_eq!(kept("12-5,"), (Some("12"), Error::Shape));
        assert_eq!(kept("192-IBC"), (Some("192"), Error::Shape));
        // A kept field keeps no reading.
        assert_eq!(Pages::read("269-38").1, Readings::default());
    }

    #[test]
    fn apart() {
        assert_eq!(
            Pages::from_parts("117", Some("26"))
                .0
                .map(|p| p.to_string()),
            Some("117-126".to_owned())
        );
        assert_eq!(
            Pages::from_parts("5", None).0.map(|p| p.to_string()),
            Some("5".to_owned())
        );
    }

    #[test]
    fn stored_and_read_back() {
        let pages = Pages::read("488-94; discussion 95-6").0.unwrap();
        let bytes = postcard::to_allocvec(&pages).unwrap();
        let back: Pages<'_> = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(back.to_string(), pages.to_string());
        assert_eq!(back.to_string(), "488-494, discussion 95-96");
    }
}
