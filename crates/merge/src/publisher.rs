//! Publisher ids: what a publisher calls an article besides its DOI
//! (merge plan §3.1) — Elsevier's PII in PubMed's `ArticleId IdType="pii"`
//! and `ELocationID EIdType="pii"`, Crossref's non-DOI `alternative-id`.
//!
//! A publisher id is written exactly as the publisher wrote it. Nothing is
//! case-folded and no punctuation is stripped, because punctuation is part
//! of an identifier and not decoration of it: `00005131-990000000-00627`
//! is not `00005131990000000627`. The one exception is Elsevier's printed
//! PII, which the world writes two ways — punctuated
//! `S0006-3495(25)00123-4` and bare `S0006349525001234` — and which we
//! write bare. On the normalized sample of 2026-09-19 both spellings occur
//! for one article on 8,350 DOIs, and the split is by source rather than
//! by publisher: PubMed writes the punctuated form (51,120 rows), Crossref
//! the bare one (8,801), 8,345 of the 8,350 that way round and 5 the
//! other, and no source ever writes both spellings for one article. Every
//! one of the 8,350 agrees once rewritten, no normalised id is owned by
//! more than one article, and no rewritten value collides with an
//! unrelated one.
//!
//! The rewrite is decided by the value's shape, under whichever
//! registrant issued it (the user, 2026-09-28): a value written in the
//! printed PII's punctuated spelling is the PII its bare spelling is,
//! because the two differ only by punctuation at fixed places, and the
//! PII is one format whoever prints it. Until then it was gated on a
//! checked-in list of Elsevier's registrants (the user, 2026-09-19),
//! grown one registrant at a time on a survey's evidence, so that another
//! publisher's lookalike would not be rewritten. Crossref's whole file
//! showed what the gate cost. Journals Elsevier produces under the prefix
//! they had before — the Journal of Pharmaceutical Sciences under
//! 10.1002, Chest under 10.1378, the British Journal of Anaesthesia under
//! 10.1093 — carry PubMed's punctuated PII beside Crossref's bare one.
//! Off the list, the two were two names, so the two records of one
//! article disagreed on an identifier, and the title test that decides
//! such a conflict split them (the survey of 2026-09-28,
//! `ol-rht-merge-survey-v10`). Nothing is rewritten where it is stored
//! (below), so the shape costs only what the list was guarding against:
//! a punctuated value under any registrant now renders bare.
//!
//! A publisher id is scoped by the registrant that issued it, so a record
//! with no DOI has nothing to scope one by and asserts no name through it:
//! 344 records of the sample (0.335 %), all PubMed's, and only 22 of their
//! 414 values are even Elsevier-shaped, so the rule costs next to nothing.
//!
//! The rules here are about a value on its own. Whether a value that looks
//! like an identifier *is* one is a fact about the whole run, and lives in
//! the identity pass: a publisher id one source claims for more than one
//! of its own records names no article (`Identity::settle`, merge plan
//! §3.3).

use std::borrow::Cow;
use std::fmt;

use model::{Doi, Registrant};
use smallvec::SmallVec;

use crate::name::Name;

/// The longest publisher id worth keeping. The longest observed is the
/// 26 characters of a book PII, `B978-0-12-804281-6.00006-9`; the bound is
/// what keeps a PII slot holding a whole citation string out of the
/// namespace when that string happens to carry no space.
pub const MAX: usize = 64;

/// Whether `value` is written in the printed PII, either spelling.
#[must_use]
pub fn pii_shaped(value: &str) -> bool {
    punctuated_pii(value) || bare_pii(value)
}

/// `S0006-3495(25)00123-4`: `^S\d{4}-\d{3}[\dXx]\(\d{2}\)\d{5}-[\dXx]$`.
fn punctuated_pii(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 21
        && b[0] == b'S'
        && b[1..5].iter().all(u8::is_ascii_digit)
        && b[5] == b'-'
        && b[6..9].iter().all(u8::is_ascii_digit)
        && is_check(b[9])
        && b[10] == b'('
        && b[11..13].iter().all(u8::is_ascii_digit)
        && b[13] == b')'
        && b[14..19].iter().all(u8::is_ascii_digit)
        && b[19] == b'-'
        && is_check(b[20])
}

/// `S0006349525001234`: `^S\d{7}[\dXx]\d{7}[\dXx]$`.
fn bare_pii(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 17
        && b[0] == b'S'
        && b[1..8].iter().all(u8::is_ascii_digit)
        && is_check(b[8])
        && b[9..16].iter().all(u8::is_ascii_digit)
        && is_check(b[16])
}

/// A PII check character: a digit or `X`, in either case as written.
const fn is_check(b: u8) -> bool {
    b.is_ascii_digit() || b == b'X' || b == b'x'
}

/// Whether `value` is rewritten: written in the printed PII's punctuated
/// spelling, under whichever registrant. The whole shape is required, so
/// a book PII's hyphens (`B978-0-12-804281-6.00006-9`) stay.
#[must_use]
pub fn rewrites(value: &str) -> bool {
    punctuated_pii(value)
}

/// The punctuation the printed PII carries and the bare spelling drops.
const PRINTED: [char; 3] = ['-', '(', ')'];

/// `value`'s canonical spelling: itself, borrowed, unless it is a
/// punctuated PII, which is written bare.
#[must_use]
pub fn canonical(value: &str) -> Cow<'_, str> {
    if rewrites(value) {
        Cow::Owned(value.chars().filter(|c| !PRINTED.contains(c)).collect())
    } else {
        Cow::Borrowed(value)
    }
}

/// The canonical spelling as bytes, without allocating: what the name's
/// equality and hash run over, so that the two spellings of one article's
/// PII are one name however they were written.
pub(crate) fn canonical_bytes(value: &str) -> impl Iterator<Item = u8> + '_ {
    let strip = rewrites(value);
    value
        .bytes()
        .filter(move |b| !(strip && matches!(b, b'-' | b'(' | b')')))
}

/// Why a publisher id is no name of ours. Every one is a fact about the
/// value, counted by the surveys; none is a failure of the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Refusal {
    /// The record holds no DOI, so nothing says which publisher issued
    /// the id and the namespace it belongs in does not exist.
    NoRegistrant,
    /// Empty.
    Empty,
    /// Longer than [`MAX`]: a PII slot holding prose, not an identifier.
    TooLong,
    /// A byte outside printable ASCII, or a space: `Läkartidningen. 2026;
    /// 123:25102-` is a citation, not an identifier.
    NotPrintable,
    /// No letter and no digit anywhere in it.
    Nothing,
    /// A bare article number — `320`, `4275`, `e112`. Two thousand one
    /// hundred and thirty-seven of the 2,453 publisher ids the census
    /// found on more than one work were these: a journal's article
    /// numbers restart, so the value names a work only once a journal
    /// qualifies it (merge plan §3.1). Evidence for the matching stage,
    /// never a name.
    ///
    /// The identity pass's collision rule (§3.3, `Identity::settle`)
    /// would catch these too, since a number a journal reuses is claimed
    /// by two of one source's records. This rule is kept in front of it
    /// for two reasons that the collision rule cannot give: it kept
    /// 31,506 publisher-id rows off the normalize surveys of 2026-09-19
    /// (28,724 PubMed, 2,782 Crossref) and so out of the name arena,
    /// which is where the merge's memory goes; and its verdict does not
    /// depend on what else is in the run, where the collision rule's
    /// does — an article number that happens to be unique in today's
    /// corpus would be a name today and a refusal tomorrow, and a name
    /// that comes and goes churns the registry and the public ids.
    ArticleNumber,
    /// A DOI. The record's own DOI restated is no publisher id, and a DOI
    /// that is not the record's is a name in the DOI namespace or nothing
    /// — merge plan §11, item 9 — but either way not this.
    Doi,
    /// The record's own DOI again, in the publisher's spelling: the value
    /// equals the DOI or its suffix once both have lost the printed PII's
    /// punctuation and their case. The commonest refusal by far — 45,802
    /// of PubMed's 135,073 publisher-id rows and 5,520 of Crossref's
    /// 11,945 on the survey of 2026-09-19 — and 786 rows of the sample
    /// are restatements only after the rewrite.
    DoiRestated,
    /// Markup in the value: `IdType="pii">JEU342`, an attribute and a `>`
    /// leaked in front of the id in NLM's own file, or a whole empty
    /// element, `<ARTICLE_ID IdType=""/>`. 388 of the 17.9 M article PIIs
    /// of PubMed's 2026 baseline, from seven publishers
    /// (`ol-rht-id-markup-v1`, 2026-09-24); none in any DOI slot or
    /// reference. What follows the leak is not to be trusted to be the id
    /// whole (the user, 2026-09-24), so the value is a malformed claim:
    /// kept as written in the normalized record, never a name.
    Markup,
}

impl Refusal {
    /// A word for the surveys to count by.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::NoRegistrant => "no registrant",
            Self::Empty => "empty",
            Self::TooLong => "too long",
            Self::NotPrintable => "not printable ASCII",
            Self::Nothing => "no letter or digit",
            Self::ArticleNumber => "an article number",
            Self::Doi => "a DOI",
            Self::DoiRestated => "the record's own DOI",
            Self::Markup => "markup in the value",
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.reason())
    }
}

/// What scopes a record's publisher ids (merge plan §3.1): the registrant
/// that keys them, and the DOIs the restatement test compares them
/// against. Built once per record from the DOI slots it filled.
#[derive(Debug, Clone, Default)]
pub struct Scope<'a> {
    registrant: Option<Registrant>,
    dois: SmallVec<[&'a str; 2]>,
}

impl<'a> Scope<'a> {
    /// A record with no DOI yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a DOI the record holds. Two DOIs whose registrants differ
    /// tie-break by the lower registrant (the user, 2026-09-19), so that
    /// however many DOI slots a record fills its publisher ids land in
    /// one namespace and a re-run picks the same one.
    pub fn doi(&mut self, doi: &'a Doi<'_>) {
        let registrant = doi.registrant();
        self.registrant = Some(
            self.registrant
                .map_or(registrant, |had| had.min(registrant)),
        );
        let text = doi.as_str();
        if !self.dois.contains(&text) {
            self.dois.push(text);
        }
    }

    /// The registrant the record's publisher ids are keyed by, if any.
    #[must_use]
    pub fn registrant(&self) -> Option<Registrant> {
        self.registrant
    }

    /// The name `value` is in this record's scope, or why it is none.
    ///
    /// The order matters. The rewrite of [`canonical`] runs before the
    /// restatement test, because 786 rows of the sample restate their own
    /// DOI only once rewritten, and the only two apparent cross-source
    /// disagreements in the whole sample are this rule failing on a
    /// punctuation variant: on `10.1016/s0002-8703(99)70115-8` Crossref's
    /// alternative id is the DOI's suffix unpunctuated and PubMed's PII is
    /// the same suffix punctuated, and testing before the rewrite keeps
    /// the second and drops the first.
    ///
    /// # Errors
    ///
    /// See [`Refusal`]: a record with no DOI, a value that is not an
    /// identifier, and the record's own DOI restated.
    pub fn name<'v>(&self, value: &'v str) -> Result<Name<'v>, Refusal> {
        let Some(registrant) = self.registrant else {
            return Err(Refusal::NoRegistrant);
        };
        shape(value)?;
        // Here and not in `shape`, which `check` shares with the reading of
        // names back from the registry: a rule a registry line could fail
        // makes a log written before it unreadable (2026-09-19). A name of
        // this kind written before the rule reads as it was, and the next
        // complete merge drops it, no record asserting it any more.
        if value.contains("IdType=") {
            return Err(Refusal::Markup);
        }
        let canonical = canonical(value);
        if self.dois.iter().any(|doi| restates(doi, &canonical)) {
            return Err(Refusal::DoiRestated);
        }
        // After the restatement test, so that a record's own DOI written
        // out in full is counted as the restatement it is rather than as
        // some other DOI. What is left is genuinely another work's DOI in
        // a publisher-id slot, which §11 item 9 has not settled: 27 rows
        // of PubMed's and 7 of Crossref's on the survey of 2026-09-19.
        if Doi::parse(value).is_ok() {
            return Err(Refusal::Doi);
        }
        Ok(Name::Publisher(registrant, value))
    }
}

/// A value that has no record to be read against: the shape rules, then
/// the DOI rule, which is what [`Name::parse`] can ask without one.
pub(crate) fn check(value: &str) -> Result<(), Refusal> {
    shape(value)?;
    if Doi::parse(value).is_ok() {
        return Err(Refusal::Doi);
    }
    Ok(())
}

/// What a publisher id has to look like to be an identifier at all.
/// Values that are not Elsevier-shaped are 61 % of the rows, and the
/// shapes among them are the ones the sample holds: bare article numbers,
/// Wolters Kluwer's `00005131-990000000-00627`, book PIIs, `cvj_12_1266`,
/// and one PII slot holding a whole citation.
fn shape(value: &str) -> Result<(), Refusal> {
    if value.is_empty() {
        return Err(Refusal::Empty);
    }
    if value.len() > MAX {
        return Err(Refusal::TooLong);
    }
    // The DOI suffix's rule (model's `doi`): printable ASCII, no space.
    if !value.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        return Err(Refusal::NotPrintable);
    }
    if !value.bytes().any(|b| b.is_ascii_alphanumeric()) {
        return Err(Refusal::Nothing);
    }
    if is_article_number(value) {
        return Err(Refusal::ArticleNumber);
    }
    Ok(())
}

/// `320`, `4275`, `e112`: a number, with or without the `e` an electronic
/// location prints in front of it.
fn is_article_number(value: &str) -> bool {
    let digits = value.strip_prefix(['e', 'E']).unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Whether `candidate` is `doi` restated: the whole DOI, or its suffix,
/// once both have lost the printed PII's punctuation and their case.
/// Case folds here and nowhere else — the DOI is lowercase by definition
/// and the publisher prints its PII in capitals, so a test that did not
/// fold would never fire.
fn restates(doi: &str, candidate: &str) -> bool {
    let suffix = doi.split_once('/').map_or(doi, |(_, s)| s);
    folded(doi).eq(folded(candidate)) || folded(suffix).eq(folded(candidate))
}

fn folded(s: &str) -> impl Iterator<Item = u8> + '_ {
    s.bytes()
        .filter(|b| !matches!(b, b'-' | b'(' | b')'))
        .map(|b| b.to_ascii_lowercase())
}

/// The registrant a `reg1016` namespace names, without allocating.
/// `None` for anything the model's own rule refuses — four to nine digits
/// with no leading zero.
pub(crate) fn registrant_of(digits: &str) -> Option<Registrant> {
    const PREFIX: &[u8] = b"10.";
    if digits.len() > 9 {
        return None;
    }
    let mut buf = [0u8; PREFIX.len() + 9];
    buf[..PREFIX.len()].copy_from_slice(PREFIX);
    buf[PREFIX.len()..PREFIX.len() + digits.len()].copy_from_slice(digits.as_bytes());
    let text = std::str::from_utf8(&buf[..PREFIX.len() + digits.len()]).ok()?;
    Registrant::parse(text).ok()
}

#[cfg(test)]
mod tests {
    use model::Doi;

    use super::{MAX, Refusal, Scope, canonical, pii_shaped, registrant_of};
    use crate::name::Name;

    fn scoped<'a>(dois: &'a [Doi<'a>]) -> Scope<'a> {
        let mut scope = Scope::new();
        for doi in dois {
            scope.doi(doi);
        }
        scope
    }

    #[test]
    fn both_spellings_of_one_pii_are_one_name() {
        let dois = [Doi::parse("10.1016/j.bpj.2025.01.001").unwrap()];
        let scope = scoped(&dois);
        let punctuated = scope.name("S0006-3495(25)00123-4").unwrap();
        let bare = scope.name("S0006349525001234").unwrap();
        assert_eq!(punctuated, bare, "one article, one name");
        assert_eq!(punctuated.to_string(), "reg1016:S0006349525001234");
        assert_eq!(bare.to_string(), "reg1016:S0006349525001234");
        assert_eq!(punctuated.to_owned(), bare.to_owned());
        // The rewrite is idempotent, so a name read back rewrites to itself.
        assert_eq!(canonical("S0006349525001234"), "S0006349525001234");
    }

    #[test]
    fn a_pii_is_one_name_in_either_spelling_under_any_registrant() {
        // 10.5555 is nobody's publisher, and the shape decides.
        let dois = [Doi::parse("10.5555/x").unwrap()];
        let scope = scoped(&dois);
        let punctuated = scope.name("S0006-3495(25)00123-4").unwrap();
        let bare = scope.name("S0006349525001234").unwrap();
        assert_eq!(punctuated, bare);
        assert_eq!(punctuated.to_string(), "reg5555:S0006349525001234");
        assert!(pii_shaped("S0006-3495(25)00123-4") && pii_shaped("S0006349525001234"));
        // Only the whole shape: a book PII keeps its hyphens.
        let book = scope.name("B978-0-12-804281-6.00006-9").unwrap();
        assert_eq!(book.to_string(), "reg5555:B978-0-12-804281-6.00006-9");
    }

    #[test]
    fn the_restatement_test_runs_after_the_rewrite() {
        // The sample's only two apparent cross-source disagreements.
        let dois = [Doi::parse("10.1016/s0002-8703(99)70115-8").unwrap()];
        let scope = scoped(&dois);
        assert_eq!(
            scope.name("S0002-8703(99)70115-8"),
            Err(Refusal::DoiRestated),
            "PubMed's punctuated PII is the DOI suffix"
        );
        assert_eq!(
            scope.name("S0002870399701158"),
            Err(Refusal::DoiRestated),
            "Crossref's bare one is the same suffix"
        );
        // The whole DOI restated, which needs no rewrite at all, is
        // counted as the restatement it is; some other DOI in the slot
        // is a DOI, and a DOI is never a publisher id (§11, item 9).
        let dois = [Doi::parse("10.1016/j.x.2025.01.001").unwrap()];
        let scope = scoped(&dois);
        assert_eq!(
            scope.name("10.1016/j.x.2025.01.001"),
            Err(Refusal::DoiRestated)
        );
        assert_eq!(scope.name("10.1038/nature12373"), Err(Refusal::Doi));
    }

    #[test]
    fn no_registrant_no_name() {
        let scope = Scope::new();
        assert_eq!(scope.registrant(), None);
        assert_eq!(scope.name("S0006349525001234"), Err(Refusal::NoRegistrant));
        assert_eq!(scope.name("cvj_12_1266"), Err(Refusal::NoRegistrant));
    }

    #[test]
    fn two_dois_tie_break_by_the_lower_registrant() {
        let dois = [
            Doi::parse("10.1053/j.x").unwrap(),
            Doi::parse("10.1016/j.y").unwrap(),
        ];
        let scope = scoped(&dois);
        assert_eq!(scope.registrant().map(model::Registrant::get), Some(1016));
        assert_eq!(
            scope.name("S0006-3495(25)00123-4").unwrap().to_string(),
            "reg1016:S0006349525001234"
        );
        // Either DOI restated is still a restatement, whole or suffix.
        assert_eq!(scope.name("10.1053/j.x"), Err(Refusal::DoiRestated));
        assert_eq!(scope.name("j.x"), Err(Refusal::DoiRestated));
        assert_eq!(scope.name("j.y"), Err(Refusal::DoiRestated));
    }

    #[test]
    fn what_a_publisher_id_is_not() {
        let dois = [Doi::parse("10.1016/j.x").unwrap()];
        let scope = scoped(&dois);
        let long = "x".repeat(MAX + 1);
        for (value, why) in [
            ("320", Refusal::ArticleNumber),
            ("4275", Refusal::ArticleNumber),
            ("e112", Refusal::ArticleNumber),
            ("E0123", Refusal::ArticleNumber),
            ("", Refusal::Empty),
            ("---", Refusal::Nothing),
            ("Läkartidningen. 2026;123:25102-", Refusal::NotPrintable),
            ("a b", Refusal::NotPrintable),
            (long.as_str(), Refusal::TooLong),
        ] {
            assert_eq!(scope.name(value), Err(why), "{value:?}");
        }
        // And what it is: kept exactly as written, punctuation and all.
        for value in [
            "cvj_12_1266",
            "00005131-990000000-00627",
            "B978-0-12-804281-6.00006-9",
            "S2213-5766(20)30238-4",
            "wst_2021_260",
        ] {
            let name = scope.name(value).unwrap();
            let written = if value == "S2213-5766(20)30238-4" {
                "S2213576620302384" // a PII, so bare
            } else {
                value
            };
            assert_eq!(name.to_string(), format!("reg1016:{written}"), "{value:?}");
            assert_eq!(Name::parse(&name.to_string()).unwrap(), name);
        }
    }

    /// The registrants the checked-in list left off, each with an
    /// article PubMed writes punctuated and Crossref bare. The first two
    /// are the survey of 2026-09-19's evidence (10.1006, 10.1074), the
    /// third is Springer's prefix, which that survey left off too
    /// (10.1007), and the last two are records the survey of Crossref's
    /// whole file split for it, the title test deciding what the two
    /// spellings had made a conflict (`ol-rht-merge-survey-v10`, PMIDs
    /// 5589 and 7437). One name each, rendered bare.
    #[test]
    fn the_registrants_the_list_left_off_are_one_name_in_either_spelling() {
        for (doi, punctuated, bare) in [
            (
                "10.1006/jmbi.1998.2346",
                "S0022-2836(98)92346-4",
                "S0022283698923464",
            ),
            (
                "10.1074/jbc.273.47.31222",
                "S0021-9258(19)59147-5",
                "S0021925819591475",
            ),
            (
                "10.1007/s11605-012-1859-9",
                "S1091-255X(23)03813-1",
                "S1091255X23038131",
            ),
            (
                "10.1002/jps.2600650416",
                "S0022-3549(15)40682-3",
                "S0022354915406823",
            ),
            (
                "10.1378/chest.70.2.315",
                "S0012-3692(16)53342-X",
                "S001236921653342X",
            ),
        ] {
            let dois = [Doi::parse(doi).unwrap()];
            let scope = scoped(&dois);
            let written = scope.name(punctuated).unwrap();
            let read = scope.name(bare).unwrap();
            assert_eq!(written, read, "{doi}");
            let registrant = scope.registrant().unwrap().get();
            assert_eq!(written.to_string(), format!("reg{registrant}:{bare}"));
            // Both still round-trip, which is what lets the registry keep
            // the names it sealed in either spelling.
            for name in [&written, &read] {
                assert_eq!(Name::parse(&name.to_string()).unwrap(), *name);
            }
        }
    }

    #[test]
    fn registrants_read_back_from_their_namespace() {
        assert_eq!(
            registrant_of("1016").map(model::Registrant::get),
            Some(1016)
        );
        assert_eq!(
            registrant_of("123456789").map(model::Registrant::get),
            Some(123_456_789)
        );
        for bad in ["", "123", "0123", "1234567890", "10a4", "１０１６"] {
            assert_eq!(registrant_of(bad), None, "{bad:?}");
        }
    }

    /// The markup NLM shipped inside a PII (ingestion plan §8 item 9,
    /// 2026-09-24) is a malformed claim, never a name — but a name of that
    /// shape already in a registry still reads back.
    #[test]
    fn markup_in_a_value_is_no_name_and_an_old_one_still_reads() {
        let doi = Doi::parse("10.1111/j.1550-7408.2008.00342.x").unwrap();
        let mut scope = Scope::new();
        scope.doi(&doi);
        assert_eq!(scope.name("IdType=\"pii\">JEU342"), Err(Refusal::Markup));
        assert!(scope.name("JEU342").is_ok());
        assert_eq!(
            scope.name("<ARTICLE_ID IdType=\"\"/>"),
            Err(Refusal::NotPrintable),
            "the empty element has a space, and the shape rule had it already"
        );
        assert!(super::check("IdType=\"pii\">JEU342").is_ok());
        assert!(Name::parse("reg1111:IdType=\"pii\">JEU342").is_ok());
    }
}
