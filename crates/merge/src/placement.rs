//! Where a record says the work was published, and the one the merge
//! promotes (citation fields plan §4 and CF2; merge version 17): the
//! journal, by its ids, the chain's top level (D14); the levels below it,
//! outermost first, as the record's labels read (D12, D13); where the work
//! sits in the innermost one (D8); and the issue's date (D4).
//!
//! Records' placements join by CF2's rules (the user, 2026-09-30). They
//! are compared level by level and by first page. Where every level both
//! give is the same but for notation (`06` and `6`), each part comes from
//! the first record that gives it, so a level only one gives is added
//! (D2); a different last page or year is a disagreement kept beside the
//! value, and does not stop the join. Where a level, the journal or the
//! first page differs, the first record's whole placement stands (D3) and
//! the others' values are its disagreements: levels of records that
//! disagree are never mixed. The records come first to last as [`ORDER`]
//! ranks their sources.

use std::borrow::Cow;

use model::Issn;
use model::date::Bound;
use model::label::{self, Label, LevelKind};
use model::pages::{self, Pages};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::SourceId;
use crate::record::{Field, Other};
use crate::view::{OwnedVenue, Venue};

/// The sources in the order their placements are taken: Crossref, then
/// PubMed, then OpenAlex (D3).
pub const ORDER: [SourceId; 3] = [SourceId::Crossref, SourceId::Pubmed, SourceId::OpenAlex];

/// A record's placement. `J` is the journal's ids: borrowed ([`Venue`])
/// in a record's view, owned ([`OwnedVenue`]) in a row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement<'a, J = Venue<'a>> {
    /// The journal, by its ids: the chain's top level (D14).
    pub journal: J,
    /// The levels below the journal, outermost first. A `Vec`, not a
    /// `SmallVec`, so that a view stays covariant in its lifetime.
    pub levels: Vec<Level<'a>>,
    /// Where the work sits in the innermost level.
    pub within: Option<Within<'a>>,
    /// The issue's date, the one a citation prints (D4; CF2's rule 5).
    pub date: Option<Bound>,
}

/// A placement that owns its text.
pub type OwnedPlacement = Placement<'static, OwnedVenue>;

/// A level below the journal: a volume, an issue, a supplement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level<'a> {
    #[allow(missing_docs)]
    pub kind: LevelKind,
    /// What it goes by, as the label read it: `45`, `3-4`, a supplement's
    /// `2`; `None` for a level named but not numbered (`Suppl`).
    pub label: Option<Cow<'a, str>>,
}

/// Where the work sits in its innermost level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Within<'a> {
    /// Pages read (D8): one range or several, in the source's order.
    Pages(Vec<Range<'a>>),
    /// Pages the grammar did not read whole: the first page where it is
    /// certain, and the field as written.
    Written {
        #[allow(missing_docs)]
        first: Option<Cow<'a, str>>,
        #[allow(missing_docs)]
        raw: Cow<'a, str>,
    },
    /// A page the source names, not numbers: `IFC`.
    Named(Cow<'a, str>),
    /// An article number, where the source marks one (Crossref's
    /// `article-number`) and gives no pages.
    ArticleNumber(Cow<'a, str>),
}

/// One range of pages, or one page, as [`model::pages::Range`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range<'a> {
    /// Words before the pages: `discussion`.
    pub label: Option<Cow<'a, str>>,
    #[allow(missing_docs)]
    pub first: Cow<'a, str>,
    /// The last page, written whole.
    pub last: Option<Cow<'a, str>>,
    /// Words after: `contd`, `+`.
    pub note: Option<Cow<'a, str>>,
}

impl<'a> Placement<'a> {
    /// A record's placement from its fields as the model reads them: the
    /// levels of its volume, then of its issue, then a supplement written
    /// before its pages; a level the issue repeats from the volume once.
    /// A label that says the work awaits its issue is no level (CF2's rule
    /// 3).
    #[must_use]
    pub fn of<'l: 'a>(
        journal: Venue<'a>,
        volume: Option<&Label<'l>>,
        issue: Option<&Label<'l>>,
        level_in_pages: Option<label::Level<'l>>,
        within: Option<Within<'a>>,
        date: Option<Bound>,
    ) -> Self {
        let mut levels: Vec<Level<'a>> = Vec::new();
        let read = volume
            .map(|v| v.levels(LevelKind::Volume))
            .into_iter()
            .flatten()
            .chain(
                issue
                    .map(|i| i.levels(LevelKind::Issue))
                    .into_iter()
                    .flatten(),
            )
            .chain(level_in_pages.filter(|l| l.kind == LevelKind::Supplement));
        for level in read {
            let level = Level {
                kind: level.kind,
                label: level.label.map(Cow::Borrowed),
            };
            let repeated = levels
                .iter()
                .any(|l| l.kind == level.kind && same_label(l, &level));
            if !repeated {
                levels.push(level);
            }
        }
        Self {
            journal,
            levels,
            within,
            date,
        }
    }

    /// The placement owning its text, the journal's NLM id as `nlm`.
    #[must_use]
    pub fn to_owned(&self, nlm: Option<Box<str>>) -> OwnedPlacement {
        Placement {
            journal: OwnedVenue {
                openalex: self.journal.openalex,
                issns: self.journal.issns.clone(),
                nlm,
            },
            levels: self.levels.iter().map(Level::to_owned).collect(),
            within: self.within.as_ref().map(Within::to_owned),
            date: self.date,
        }
    }
}

impl OwnedPlacement {
    /// The placement, borrowed.
    #[must_use]
    pub fn borrow(&self) -> Placement<'_> {
        Placement {
            journal: self.journal.borrow(),
            levels: self.levels.iter().map(Level::borrow).collect(),
            within: self.within.as_ref().map(Within::borrow),
            date: self.date,
        }
    }

    /// No part at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.journal.is_empty()
            && self.levels.is_empty()
            && self.within.is_none()
            && self.date.is_none()
    }
}

impl Level<'_> {
    /// The level owning its text.
    #[must_use]
    pub fn to_owned(&self) -> Level<'static> {
        Level {
            kind: self.kind,
            label: self.label.as_deref().map(|l| Cow::Owned(l.to_owned())),
        }
    }

    /// The level, borrowed.
    #[must_use]
    pub fn borrow(&self) -> Level<'_> {
        Level {
            kind: self.kind,
            label: self.label.as_deref().map(Cow::Borrowed),
        }
    }
}

impl<'a> Within<'a> {
    /// Where the work sits, from a page field as the model reads it.
    #[must_use]
    pub fn of_pages<'p: 'a>(pages: &Pages<'p>) -> Self {
        match pages {
            Pages::Read(ranges) => Self::Pages(
                ranges
                    .iter()
                    .map(|r: &pages::Range<'p>| Range {
                        label: r.label.map(Cow::Borrowed),
                        first: Cow::Borrowed(r.first),
                        last: r.last.clone(),
                        note: r.note.map(Cow::Borrowed),
                    })
                    .collect(),
            ),
            Pages::Kept { first, raw, .. } => Self::Written {
                first: first.map(Cow::Borrowed),
                raw: Cow::Borrowed(raw),
            },
            Pages::Named(name) => Self::Named(Cow::Borrowed(name)),
        }
    }

    /// The first page, or the name or the number, as written: what two
    /// records are compared by.
    #[must_use]
    pub fn first(&self) -> Option<&str> {
        match self {
            Self::Pages(ranges) => ranges.first().map(|r| &*r.first),
            Self::Written { first, .. } => first.as_deref(),
            Self::Named(name) => Some(name),
            Self::ArticleNumber(number) => Some(number),
        }
    }

    /// The last page of the first range, where one is given.
    #[must_use]
    pub fn last(&self) -> Option<&str> {
        match self {
            Self::Pages(ranges) => ranges.first().and_then(|r| r.last.as_deref()),
            _ => None,
        }
    }

    /// Where the work sits, owning its text.
    #[must_use]
    pub fn to_owned(&self) -> Within<'static> {
        let own = |s: &Cow<'_, str>| Cow::Owned(s.to_string());
        match self {
            Self::Pages(ranges) => Within::Pages(
                ranges
                    .iter()
                    .map(|r| Range {
                        label: r.label.as_ref().map(own),
                        first: own(&r.first),
                        last: r.last.as_ref().map(own),
                        note: r.note.as_ref().map(own),
                    })
                    .collect(),
            ),
            Self::Written { first, raw } => Within::Written {
                first: first.as_ref().map(own),
                raw: own(raw),
            },
            Self::Named(name) => Within::Named(own(name)),
            Self::ArticleNumber(number) => Within::ArticleNumber(own(number)),
        }
    }

    /// Where the work sits, borrowed.
    #[must_use]
    pub fn borrow(&self) -> Within<'_> {
        match self {
            Self::Pages(ranges) => Within::Pages(
                ranges
                    .iter()
                    .map(|r| Range {
                        label: r.label.as_deref().map(Cow::Borrowed),
                        first: Cow::Borrowed(&r.first),
                        last: r.last.as_deref().map(Cow::Borrowed),
                        note: r.note.as_deref().map(Cow::Borrowed),
                    })
                    .collect(),
            ),
            Self::Written { first, raw } => Within::Written {
                first: first.as_deref().map(Cow::Borrowed),
                raw: Cow::Borrowed(raw),
            },
            Self::Named(name) => Within::Named(Cow::Borrowed(name)),
            Self::ArticleNumber(number) => Within::ArticleNumber(Cow::Borrowed(number)),
        }
    }
}

impl std::fmt::Display for Within<'_> {
    /// For reports and the API's disagreements: ranges as `first-last`,
    /// with their words, joined by `, `; the rest as written.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pages(ranges) => {
                for (i, r) in ranges.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    if let Some(label) = &r.label {
                        write!(f, "{label} ")?;
                    }
                    f.write_str(&r.first)?;
                    if let Some(last) = &r.last {
                        write!(f, "-{last}")?;
                    }
                    if let Some(note) = &r.note {
                        write!(f, " {note}")?;
                    }
                }
                Ok(())
            }
            Self::Written { raw, .. } => f.write_str(raw),
            Self::Named(name) => f.write_str(name),
            Self::ArticleNumber(number) => f.write_str(number),
        }
    }
}

/// How a work's records' placements joined, as the report counts them.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum Join {
    /// One record gives a placement.
    #[default]
    One,
    /// Every part two records give is the same, and neither gives more.
    Same,
    /// The same where both give, and one gives more: added (D2).
    More,
    /// No part both give: joined with nothing to check the join by
    /// (D2, counted apart).
    Unchecked,
    /// The journal, a level or the first page differs: the first record's
    /// placement stands whole (D3).
    Conflict,
}

impl Join {
    /// The class, in words, for the report.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::One => "one record",
            Self::Same => "the same",
            Self::More => "joined: one gives more",
            Self::Unchecked => "joined: nothing shared to check by",
            Self::Conflict => "conflict: the first record's stands",
        }
    }
}

/// A work's placement as promoted: each part with the record it came
/// from, and the others' values where they differ.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promoted<'a, J = Venue<'a>> {
    /// The journal; its ISSNs those of every record that agrees.
    pub journal: Option<Field<J>>,
    /// The levels below the journal, outermost first.
    pub levels: SmallVec<[Field<Level<'a>>; 2]>,
    /// Where the work sits.
    pub within: Option<Field<Within<'a>>>,
    /// The issue's date.
    pub date: Option<Field<Bound>>,
    /// How the records joined.
    pub join: Join,
}

/// A promoted placement that owns its text.
pub type OwnedPromoted = Promoted<'static, OwnedVenue>;

impl OwnedPromoted {
    /// The promoted placement, borrowed.
    #[must_use]
    pub fn borrow(&self) -> Promoted<'_> {
        Promoted {
            journal: self.journal.as_ref().map(|j| j.map(OwnedVenue::borrow)),
            levels: self.levels.iter().map(|l| l.map(Level::borrow)).collect(),
            within: self.within.as_ref().map(|w| w.map(Within::borrow)),
            date: self.date.clone(),
            join: self.join,
        }
    }
}

impl<J> Promoted<'_, J> {
    /// The source the journal came from: the record whose journal's name
    /// a row shows.
    #[must_use]
    pub fn journal_from(&self) -> Option<SourceId> {
        self.journal.as_ref().map(|j| j.from)
    }
}

impl Promoted<'_> {
    /// The promoted placement owning its text, the journals' NLM ids
    /// resolved by `nlm`.
    #[must_use]
    pub fn to_owned(&self, nlm: impl Fn(&Venue<'_>) -> Option<Box<str>>) -> OwnedPromoted {
        let venue = |v: &Venue<'_>| OwnedVenue {
            openalex: v.openalex,
            issns: v.issns.clone(),
            nlm: nlm(v),
        };
        Promoted {
            journal: self.journal.as_ref().map(|j| j.map(venue)),
            levels: self.levels.iter().map(|l| l.map(Level::to_owned)).collect(),
            within: self.within.as_ref().map(|w| w.map(Within::to_owned)),
            date: self.date.clone(),
            join: self.join,
        }
    }
}

/// What the join found for one part, for the merge's tallies: the part,
/// the source it came from, and each other source that gave it, with
/// whether it differed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// `journal`, a level's kind (`volume`), `pages` or `issue date`.
    pub part: &'static str,
    #[allow(missing_docs)]
    pub from: SourceId,
    #[allow(missing_docs)]
    pub compared: SmallVec<[(SourceId, bool); 2]>,
}

/// The work's placement from its records' (CF2's rules 1 to 4), the
/// records first to last in precedence: a source in [`ORDER`], and a
/// promoted record before its source's others. `None` where no record
/// gives one. What was compared is pushed onto `seen`.
#[must_use]
pub fn promote(
    records: &[(SourceId, &OwnedPlacement)],
    seen: &mut Vec<Seen>,
) -> Option<OwnedPromoted> {
    let placed: SmallVec<[(SourceId, &OwnedPlacement); 3]> = records
        .iter()
        .filter(|(_, p)| !p.is_empty())
        .copied()
        .collect();
    let (&(_, base), rest) = placed.split_first()?;
    let verdicts: SmallVec<[Verdict; 2]> = rest.iter().map(|(_, p)| compare(base, p)).collect();
    let join = if rest.is_empty() {
        Join::One
    } else if verdicts.iter().any(|v| v.conflict) {
        Join::Conflict
    } else if !verdicts.iter().all(|v| v.shared) {
        Join::Unchecked
    } else if verdicts.iter().any(|v| v.more || v.less) {
        Join::More
    } else {
        Join::Same
    };
    // In a conflict the first record's placement stands whole; otherwise
    // every record joins.
    let joined: &[(SourceId, &OwnedPlacement)] = if join == Join::Conflict {
        &placed[..1]
    } else {
        &placed
    };
    let mut promoted = OwnedPromoted {
        join,
        ..OwnedPromoted::default()
    };
    promoted.journal = journal(joined, &placed, seen);
    promoted.levels = levels(joined, &placed, seen);
    promoted.within = part(
        "pages",
        joined,
        &placed,
        |p| p.within.clone(),
        |a, b| !same(a.first(), b.first()) || !same_end(a, b),
        seen,
    );
    promoted.date = part(
        "issue date",
        joined,
        &placed,
        |p| p.date,
        |a, b| a.year != b.year,
        seen,
    );
    Some(promoted)
}

/// One record's placement against the first's.
#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)] // independent facts
struct Verdict {
    /// The journal, a level or the first page differs.
    conflict: bool,
    /// Some part both give is the same.
    shared: bool,
    /// It gives a part the first does not.
    more: bool,
    /// The first gives a part it does not.
    less: bool,
}

fn compare(base: &OwnedPlacement, other: &OwnedPlacement) -> Verdict {
    let mut v = Verdict::default();
    let (a, b) = (&base.journal.issns, &other.journal.issns);
    v.note(!a.is_empty(), !b.is_empty(), !disjoint(a, b));
    let mut done: SmallVec<[LevelKind; 4]> = SmallVec::new();
    for kind in base.levels.iter().chain(&other.levels).map(|l| l.kind) {
        if done.contains(&kind) {
            continue;
        }
        done.push(kind);
        let a = base.levels.iter().find(|l| l.kind == kind);
        let b = other.levels.iter().find(|l| l.kind == kind);
        let agree = match (a, b) {
            (Some(a), Some(b)) => {
                // A level one names and the other numbers: the other gives
                // more.
                if same_label(a, b) && a.label.is_none() != b.label.is_none() {
                    v.more |= a.label.is_none();
                    v.less |= b.label.is_none();
                }
                same_label(a, b)
            }
            _ => true,
        };
        v.note(a.is_some(), b.is_some(), agree);
    }
    let (a, b) = (base.within.as_ref(), other.within.as_ref());
    v.note(
        a.is_some(),
        b.is_some(),
        same(a.and_then(Within::first), b.and_then(Within::first)),
    );
    match (base.date, other.date) {
        (Some(a), Some(b)) => v.shared |= a.year == b.year,
        (a, b) => {
            v.more |= a.is_none() && b.is_some();
            v.less |= a.is_some() && b.is_none();
        }
    }
    v
}

impl Verdict {
    /// One part: given by the first record, by the other, and whether the
    /// two are the same where both give it.
    fn note(&mut self, first: bool, other: bool, same: bool) {
        match (first, other) {
            (true, true) if same => self.shared = true,
            (true, true) => self.conflict = true,
            (true, false) => self.less = true,
            (false, true) => self.more = true,
            (false, false) => {}
        }
    }
}

/// The journal: from the first record that gives one, its ISSNs those of
/// every record joined, deduplicated; the others' where they are disjoint.
fn journal(
    joined: &[(SourceId, &OwnedPlacement)],
    placed: &[(SourceId, &OwnedPlacement)],
    seen: &mut Vec<Seen>,
) -> Option<Field<OwnedVenue>> {
    let at = joined.iter().position(|(_, p)| !p.journal.is_empty())?;
    let (from, first) = &joined[at];
    let mut value = first.journal.clone();
    for (_, p) in joined {
        for issn in &p.journal.issns {
            if !value.issns.contains(issn) {
                value.issns.push(*issn);
            }
        }
        value.openalex = value.openalex.or(p.journal.openalex);
        if value.nlm.is_none() {
            value.nlm.clone_from(&p.journal.nlm);
        }
    }
    let mut others: SmallVec<[Other<OwnedVenue>; 2]> = SmallVec::new();
    let mut compared: SmallVec<[(SourceId, bool); 2]> = SmallVec::new();
    for (i, (source, p)) in placed.iter().enumerate() {
        if i == at || p.journal.is_empty() {
            continue;
        }
        let differs = disjoint(&value.issns, &p.journal.issns);
        compared.push((*source, differs));
        if differs {
            others.push(Other {
                source: *source,
                value: p.journal.clone(),
            });
        }
    }
    seen.push(Seen {
        part: "journal",
        from: *from,
        compared,
    });
    Some(Field {
        value,
        from: *from,
        others,
    })
}

/// The levels: the first joined record's as written, then each kind a
/// later one adds, placed before the first level of greater rank; a level
/// the first names and a later one numbers takes the number. The others'
/// values where they differ.
fn levels(
    joined: &[(SourceId, &OwnedPlacement)],
    placed: &[(SourceId, &OwnedPlacement)],
    seen: &mut Vec<Seen>,
) -> SmallVec<[Field<Level<'static>>; 2]> {
    let field = |source: SourceId, level: &Level<'static>| Field {
        value: level.clone(),
        from: source,
        others: SmallVec::new(),
    };
    let mut out: SmallVec<[Field<Level<'static>>; 2]> = SmallVec::new();
    if let Some((source, first)) = joined.first() {
        out.extend(first.levels.iter().map(|l| field(*source, l)));
    }
    for (source, p) in joined.iter().skip(1) {
        for level in &p.levels {
            if let Some(f) = out.iter_mut().find(|f| f.value.kind == level.kind) {
                if f.value.label.is_none() && level.label.is_some() {
                    f.value.label.clone_from(&level.label);
                    f.from = *source;
                }
                continue;
            }
            let at = out
                .iter()
                .position(|f| f.value.kind > level.kind)
                .unwrap_or(out.len());
            out.insert(at, field(*source, level));
        }
    }
    for promoted in &mut out {
        let mut compared: SmallVec<[(SourceId, bool); 2]> = SmallVec::new();
        let mut own = true;
        for (source, p) in placed {
            let Some(level) = p.levels.iter().find(|l| l.kind == promoted.value.kind) else {
                continue;
            };
            // The record the level came from is not compared with itself.
            if own && *source == promoted.from && level.label == promoted.value.label {
                own = false;
                continue;
            }
            let differs = !same_label(level, &promoted.value);
            compared.push((*source, differs));
            if differs {
                promoted.others.push(Other {
                    source: *source,
                    value: level.clone(),
                });
            }
        }
        seen.push(Seen {
            part: promoted.value.kind.as_str(),
            from: promoted.from,
            compared,
        });
    }
    out
}

/// One part: from the first joined record that gives it; the others'
/// values where `differs`.
fn part<T: Clone>(
    name: &'static str,
    joined: &[(SourceId, &OwnedPlacement)],
    placed: &[(SourceId, &OwnedPlacement)],
    get: impl Fn(&OwnedPlacement) -> Option<T>,
    differs: impl Fn(&T, &T) -> bool,
    seen: &mut Vec<Seen>,
) -> Option<Field<T>> {
    let (at, (from, value)) = joined
        .iter()
        .enumerate()
        .find_map(|(i, (s, p))| get(p).map(|v| (i, (*s, v))))?;
    let mut others: SmallVec<[Other<T>; 2]> = SmallVec::new();
    let mut compared: SmallVec<[(SourceId, bool); 2]> = SmallVec::new();
    for (i, (source, p)) in placed.iter().enumerate() {
        if i == at {
            continue;
        }
        let Some(other) = get(p) else {
            continue;
        };
        let d = differs(&value, &other);
        compared.push((*source, d));
        if d {
            others.push(Other {
                source: *source,
                value: other,
            });
        }
    }
    seen.push(Seen {
        part: name,
        from,
        compared,
    });
    Some(Field {
        value,
        from,
        others,
    })
}

/// Both give ISSNs and none is shared: the venue's rule of merge plan §5.
fn disjoint(a: &[Issn], b: &[Issn]) -> bool {
    !a.is_empty() && !b.is_empty() && !a.iter().any(|i| b.contains(i))
}

/// Two levels' labels the same but for notation; a level named but not
/// numbered agrees with any number.
fn same_label(a: &Level<'_>, b: &Level<'_>) -> bool {
    match (&a.label, &b.label) {
        (Some(a), Some(b)) => notation(a) == notation(b),
        _ => true,
    }
}

/// Two first pages the same but for notation; absent on either side is
/// no disagreement.
fn same(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => notation(a) == notation(b),
        _ => true,
    }
}

/// Two places' last pages the same but for notation, where both give one.
fn same_end(a: &Within<'_>, b: &Within<'_>) -> bool {
    same(a.last(), b.last())
}

/// A label or a page as compared: lower case, and each run of digits
/// without its leading zeros (`06` is `6`, `E029` is `e29`).
fn notation(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut run = String::from(c);
            while let Some(&d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                run.push(d);
                chars.next();
            }
            let trimmed = run.trim_start_matches('0');
            out.push_str(if trimmed.is_empty() { "0" } else { trimmed });
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use model::date::Year;

    use super::*;

    /// A placement from its parts: ISSNs, levels as `(kind, label)`, the
    /// first and last page, the issue's year.
    fn placed(
        issns: &[&str],
        levels: &[(LevelKind, Option<&str>)],
        pages: Option<(&str, Option<&str>)>,
        year: Option<&str>,
    ) -> OwnedPlacement {
        Placement {
            journal: OwnedVenue {
                openalex: None,
                issns: issns.iter().map(|i| Issn::parse(i).unwrap()).collect(),
                nlm: None,
            },
            levels: levels
                .iter()
                .map(|(kind, label)| Level {
                    kind: *kind,
                    label: label.map(|l| Cow::Owned(l.to_owned())),
                })
                .collect(),
            within: pages.map(|(first, last)| {
                Within::Pages(vec![Range {
                    label: None,
                    first: Cow::Owned(first.to_owned()),
                    last: last.map(|l| Cow::Owned(l.to_owned())),
                    note: None,
                }])
            }),
            date: year.map(|y| Bound {
                year: Year::parse(y).unwrap(),
                month: None,
                day: None,
            }),
        }
    }

    const V: LevelKind = LevelKind::Volume;
    const I: LevelKind = LevelKind::Issue;
    const S: LevelKind = LevelKind::Supplement;

    fn join(a: &OwnedPlacement, b: &OwnedPlacement) -> OwnedPromoted {
        let mut seen = Vec::new();
        promote(&[(SourceId::Crossref, a), (SourceId::Pubmed, b)], &mut seen).unwrap()
    }

    fn levels(p: &OwnedPromoted) -> Vec<(LevelKind, Option<&str>, SourceId)> {
        p.levels
            .iter()
            .map(|f| (f.value.kind, f.value.label.as_deref(), f.from))
            .collect()
    }

    #[test]
    fn the_same_but_for_notation() {
        let crossref = placed(
            &["0028-0836"],
            &[(V, Some("06")), (I, Some("3"))],
            Some(("123", Some("129"))),
            Some("2020"),
        );
        let pubmed = placed(
            &["0028-0836"],
            &[(V, Some("6")), (I, Some("03"))],
            Some(("123", Some("129"))),
            Some("2020"),
        );
        let p = join(&crossref, &pubmed);
        assert_eq!(p.join, Join::Same);
        assert_eq!(
            levels(&p),
            [
                (V, Some("06"), SourceId::Crossref),
                (I, Some("3"), SourceId::Crossref)
            ]
        );
        assert!(p.levels.iter().all(|l| l.others.is_empty()));
    }

    #[test]
    fn a_level_one_gives_is_added_in_its_place() {
        let crossref = placed(&["0028-0836"], &[(I, Some("3"))], Some(("123", None)), None);
        let pubmed = placed(
            &[],
            &[(V, Some("45")), (I, Some("3")), (S, None)],
            Some(("123", None)),
            Some("2020"),
        );
        let p = join(&crossref, &pubmed);
        assert_eq!(p.join, Join::More);
        assert_eq!(
            levels(&p),
            [
                (V, Some("45"), SourceId::Pubmed),
                (I, Some("3"), SourceId::Crossref),
                (S, None, SourceId::Pubmed),
            ]
        );
        assert_eq!(p.date.as_ref().map(|d| d.from), Some(SourceId::Pubmed));
    }

    #[test]
    fn a_named_level_takes_the_number_another_gives() {
        let crossref = placed(&[], &[(V, Some("20")), (S, None)], None, None);
        let pubmed = placed(&[], &[(V, Some("20")), (S, Some("2"))], None, None);
        let p = join(&crossref, &pubmed);
        assert_eq!(p.join, Join::More);
        assert_eq!(
            levels(&p),
            [
                (V, Some("20"), SourceId::Crossref),
                (S, Some("2"), SourceId::Pubmed)
            ]
        );
    }

    #[test]
    fn a_conflict_keeps_the_first_whole() {
        let crossref = placed(
            &[],
            &[(V, Some("45")), (I, Some("3"))],
            Some(("123", None)),
            None,
        );
        let pubmed = placed(
            &[],
            &[(V, Some("46")), (S, None)],
            Some(("123", None)),
            Some("2020"),
        );
        let p = join(&crossref, &pubmed);
        assert_eq!(p.join, Join::Conflict);
        // Nothing of PubMed's is joined: no supplement, no date.
        assert_eq!(
            levels(&p),
            [
                (V, Some("45"), SourceId::Crossref),
                (I, Some("3"), SourceId::Crossref)
            ]
        );
        assert_eq!(p.levels[0].others.len(), 1);
        assert_eq!(p.levels[0].others[0].value.label.as_deref(), Some("46"));
        assert_eq!(p.date, None);
    }

    #[test]
    fn a_different_first_page_is_a_conflict() {
        let crossref = placed(&[], &[(V, Some("418"))], Some(("8", None)), None);
        let pubmed = placed(&[], &[(V, Some("418"))], Some(("9", None)), None);
        let p = join(&crossref, &pubmed);
        assert_eq!(p.join, Join::Conflict);
        assert_eq!(p.within.as_ref().unwrap().others.len(), 1);
    }

    #[test]
    fn journals_whose_issns_do_not_meet_conflict() {
        let crossref = placed(&["0028-0836"], &[(V, Some("1"))], None, None);
        let pubmed = placed(&["1476-4687"], &[(V, Some("1"))], None, None);
        let p = join(&crossref, &pubmed);
        assert_eq!(p.join, Join::Conflict);
        let journal = p.journal.unwrap();
        assert_eq!(journal.from, SourceId::Crossref);
        assert_eq!(journal.others.len(), 1);
    }

    #[test]
    fn the_journal_has_every_joined_records_issns() {
        let crossref = placed(&["0028-0836"], &[(V, Some("1"))], None, None);
        let mut pubmed = placed(&["0028-0836", "1476-4687"], &[(V, Some("1"))], None, None);
        pubmed.journal.nlm = Some("0410462".into());
        let p = join(&crossref, &pubmed);
        let journal = p.journal.unwrap();
        assert_eq!(journal.value.issns.len(), 2);
        assert_eq!(journal.value.nlm.as_deref(), Some("0410462"));
        assert!(journal.others.is_empty());
    }

    #[test]
    fn nothing_shared_is_joined_and_counted_apart() {
        let crossref = placed(&[], &[], Some(("5", Some("9"))), None);
        let pubmed = placed(&[], &[(V, Some("12"))], None, None);
        let p = join(&crossref, &pubmed);
        assert_eq!(p.join, Join::Unchecked);
        assert_eq!(levels(&p), [(V, Some("12"), SourceId::Pubmed)]);
        assert!(p.within.is_some());
    }

    #[test]
    fn a_different_end_page_or_year_is_a_disagreement_not_a_conflict() {
        let crossref = placed(
            &[],
            &[(V, Some("7"))],
            Some(("129", Some("140"))),
            Some("2019"),
        );
        let pubmed = placed(
            &[],
            &[(V, Some("7"))],
            Some(("129", Some("139"))),
            Some("2020"),
        );
        let p = join(&crossref, &pubmed);
        assert_eq!(p.join, Join::Same);
        let within = p.within.unwrap();
        assert_eq!(within.value.last(), Some("140"));
        assert_eq!(within.others.len(), 1);
        assert_eq!(p.date.unwrap().others.len(), 1);
    }

    #[test]
    fn one_record_or_none() {
        let crossref = placed(&[], &[(V, Some("7"))], None, None);
        let empty = placed(&[], &[], None, None);
        let mut seen = Vec::new();
        let p = promote(
            &[(SourceId::Crossref, &crossref), (SourceId::Pubmed, &empty)],
            &mut seen,
        );
        assert_eq!(p.map(|p| p.join), Some(Join::One));
        assert!(promote(&[(SourceId::Pubmed, &empty)], &mut seen).is_none());
    }

    #[test]
    fn what_was_compared_is_seen() {
        let crossref = placed(
            &["0028-0836"],
            &[(V, Some("45"))],
            Some(("123", None)),
            None,
        );
        let pubmed = placed(
            &["0028-0836"],
            &[(V, Some("46"))],
            Some(("123", None)),
            None,
        );
        let mut seen = Vec::new();
        let _ = promote(
            &[(SourceId::Crossref, &crossref), (SourceId::Pubmed, &pubmed)],
            &mut seen,
        );
        let volume = seen.iter().find(|s| s.part == "volume").unwrap();
        assert_eq!(volume.from, SourceId::Crossref);
        assert_eq!(volume.compared.as_slice(), [(SourceId::Pubmed, true)]);
        let journal = seen.iter().find(|s| s.part == "journal").unwrap();
        assert_eq!(journal.compared.as_slice(), [(SourceId::Pubmed, false)]);
    }

    #[test]
    fn notation_is_case_and_leading_zeros() {
        assert_eq!(notation("06"), "6");
        assert_eq!(notation("E029"), "e29");
        assert_eq!(notation("0"), "0");
        assert_eq!(notation("S10-003"), "s10-3");
    }

    #[test]
    fn a_records_levels_from_its_labels() {
        let volume = Label::read("20 Suppl 2", LevelKind::Volume).0;
        let issue = Label::read("Volume 20 Issue 3", LevelKind::Issue).0;
        let p = Placement::of(
            Venue::default(),
            volume.as_ref(),
            issue.as_ref(),
            None,
            None,
            None,
        );
        let got: Vec<_> = p
            .levels
            .iter()
            .map(|l| (l.kind, l.label.as_deref()))
            .collect();
        assert_eq!(got, [(V, Some("20")), (S, Some("2")), (I, Some("3"))]);
        // Awaiting its issue: no level.
        let pp = Label::read("PP", LevelKind::Volume).0;
        let p = Placement::of(Venue::default(), pp.as_ref(), None, None, None, None);
        assert!(p.levels.is_empty());
    }

    #[test]
    fn stored_and_read_back() {
        let p = placed(
            &["0028-0836"],
            &[(V, Some("45")), (S, None)],
            Some(("123", Some("129"))),
            Some("2020"),
        );
        let bytes = postcard::to_allocvec(&p).unwrap();
        let back: OwnedPlacement = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(back, p);
        let promoted = join(&p, &p);
        let borrowed = promoted.borrow();
        let bytes = postcard::to_allocvec(&borrowed).unwrap();
        let back: Promoted<'_> = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(format!("{back:?}"), format!("{borrowed:?}"));
    }
}
