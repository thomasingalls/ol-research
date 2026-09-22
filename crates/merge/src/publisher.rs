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
//! The rewrite is gated on the publisher — the DOI registrant that issued
//! the id — and never on the shape alone (the user's call, 2026-09-19,
//! against a measurement that argued for shape): anything that went
//! through Elsevier's machinery will match Elsevier's rules, but a value
//! that merely looks like a PII is not thereby Elsevier's, and another
//! publisher's identifier must not be mangled to make ours tidier.
//! [`ELSEVIER`] is that list of registrants and [`pii_shaped`] is what
//! counts the values it is missing, so the list grows on evidence rather
//! than on a guess about who else prints a PII. Each entry carries the
//! evidence that put it there. The list decides a name's canonical
//! spelling, but no longer what is stored: a name keeps the spelling it
//! was written in and [`canonical`] is a view of it, computed where the
//! name is rendered ([`crate::name::Name`], 2026-09-19). So the list is
//! free to change, and a change costs a merge rather than a migration of
//! the registry. See [`ELSEVIER`].
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

/// The DOI registrants Elsevier deposits under, and so the publishers
/// whose printed PII is rewritten to its bare spelling.
///
/// **Extending it.** Add the registrant's digits in order with the
/// evidence that put them there. The evidence is a normalize survey
/// reporting a non-zero `pii shaped, registrant not on the Elsevier list`
/// count against that registrant: it says that a publisher we do not know
/// to be Elsevier prints Elsevier's identifier, and one of the two facts
/// has to give. Nothing is guessed here — a registrant nobody has seen
/// carrying a printed PII does not belong on the list.
///
/// **What a change costs, and why it once cost more.** A name used to be
/// stored in its canonical spelling and `Name::parse` refused one that was
/// not canonical, so putting a registrant on this list retroactively
/// unspelled every name already sealed under it: the merge of 2026-09-19
/// failed while *reading* the registry, before it did any work, on 99
/// punctuated `reg1006:`/`reg1074:` names that journal batch 6 had sealed.
/// The names are now stored as they were written and the canonical
/// spelling is a view, so a sealed name reads back whatever this list says
/// later and an entry costs a merge rather than a migration: two spellings
/// that were two names become one, which moves works together, retires
/// ids and rewrites the rows that carry them. That is a full merge, and
/// the reason `merge::VERSION` mentions this list.
///
/// **What the counter says the registrant is not.** The id is issued by
/// whoever produces the journal, and the registrant records only who
/// registered that DOI: 110 of the 1,285 journal prefixes seen under
/// 10.1016 are also seen under some other registrant, which is why this
/// list keeps growing an entry at a time. `10.1007` is the case left off
/// on the survey of 2026-09-19 — 23 PII-shaped values and 6 articles
/// written both ways, against 10.1006's 66 and 19 and 10.1074's 126 and
/// 57 — because it is Springer's prefix carrying a few journals that
/// moved to Elsevier and at least one that did not (Psychometrika's
/// `S003331230004477X` on `10.1007/bf02288892`), and because nine
/// registrants nobody has proposed carry more of the same evidence
/// (10.1038's 33 articles, 10.1136's 28, 10.1093's 19, 10.1111's 17).
/// Whether the registrant is the right gate at all is the user's to
/// answer; until it is, 1007 is not worth a merge on its own.
pub const ELSEVIER: &[u32] = &[
    // The main prefix: 35,833 Elsevier-shaped values on the sample.
    1016, // Elsevier's second prefix in the sample (the user, 2026-09-19).
    1053,
    // Off the list until the user says otherwise, the format no longer
    // being the reason (2026-09-19): 1006 (Academic Press, 26 of its 28
    // journal prefixes also issued under 10.1016 itself) and 1074
    // (ASBMB's, whose two journals — the Journal of Biological
    // Chemistry's S00219258 and Molecular & Cellular Proteomics'
    // S15359476 — Elsevier produced end to end). Adding either is a
    // decision and a merge, not a migration; the evidence is above.
];

/// The longest publisher id worth keeping. The longest observed is the
/// 26 characters of a book PII, `B978-0-12-804281-6.00006-9`; the bound is
/// what keeps a PII slot holding a whole citation string out of the
/// namespace when that string happens to carry no space.
pub const MAX: usize = 64;

/// Whether `registrant` is one of Elsevier's (see [`ELSEVIER`]).
#[must_use]
pub fn is_elsevier(registrant: Registrant) -> bool {
    ELSEVIER.contains(&registrant.get())
}

/// Whether `value` is written in Elsevier's printed PII, either spelling.
///
/// This is the shape test of the off-list counter and nothing else: a
/// value of this shape is Elsevier's identifier only when its registrant
/// says so, which is what [`is_elsevier`] decides.
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

/// Whether `value` under `registrant` is rewritten: Elsevier's, and
/// written in the punctuated spelling. Both halves are required — the
/// list without the shape would strip the hyphens out of a book PII, the
/// shape without the list would rewrite another publisher's lookalike.
#[must_use]
pub fn rewrites(registrant: Registrant, value: &str) -> bool {
    is_elsevier(registrant) && punctuated_pii(value)
}

/// The punctuation the printed PII carries and the bare spelling drops.
const PRINTED: [char; 3] = ['-', '(', ')'];

/// `value`'s canonical spelling under `registrant`: itself, borrowed,
/// unless it is Elsevier's punctuated PII, which is written bare.
#[must_use]
pub fn canonical(registrant: Registrant, value: &str) -> Cow<'_, str> {
    if rewrites(registrant, value) {
        Cow::Owned(value.chars().filter(|c| !PRINTED.contains(c)).collect())
    } else {
        Cow::Borrowed(value)
    }
}

/// The canonical spelling as bytes, without allocating: what the name's
/// equality and hash run over, so that the two spellings of one article's
/// PII are one name however they were written.
pub(crate) fn canonical_bytes(
    registrant: Registrant,
    value: &str,
) -> impl Iterator<Item = u8> + '_ {
    let strip = rewrites(registrant, value);
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
        let canonical = canonical(registrant, value);
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

    use super::{ELSEVIER, MAX, Refusal, Scope, canonical, is_elsevier, pii_shaped, registrant_of};
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
        let registrant = scope.registrant().unwrap();
        assert_eq!(
            canonical(registrant, "S0006349525001234"),
            "S0006349525001234"
        );
    }

    #[test]
    fn a_lookalike_under_another_registrant_is_left_alone() {
        // 10.5555 is nobody's Elsevier prefix, so its lookalike keeps the
        // punctuation the publisher printed.
        let dois = [Doi::parse("10.5555/x").unwrap()];
        let scope = scoped(&dois);
        let punctuated = scope.name("S0006-3495(25)00123-4").unwrap();
        let bare = scope.name("S0006349525001234").unwrap();
        assert_ne!(punctuated, bare, "not ours to merge");
        assert_eq!(punctuated.to_string(), "reg5555:S0006-3495(25)00123-4");
        assert!(pii_shaped("S0006-3495(25)00123-4"), "counted as off-list");
        assert!(!is_elsevier(Doi::parse("10.5555/x").unwrap().registrant()));
        // Every registrant on the list does rewrite it.
        for registrant in ELSEVIER {
            let text = format!("10.{registrant}/x");
            let doi = Doi::parse(&text).unwrap();
            let mut scope = Scope::new();
            scope.doi(&doi);
            assert_eq!(
                scope.name("S0006-3495(25)00123-4").unwrap().to_string(),
                format!("reg{registrant}:S0006349525001234")
            );
        }
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
                "S2213576620302384" // Elsevier's, so bare
            } else {
                value
            };
            assert_eq!(name.to_string(), format!("reg1016:{written}"), "{value:?}");
            assert_eq!(Name::parse(&name.to_string()).unwrap(), name);
        }
    }

    #[test]
    fn the_registrants_the_survey_of_2026_09_19_found_are_still_off_the_list() {
        // The survey's evidence is real — 66 PII-shaped values under
        // 10.1006 and 126 under 10.1074, 19 and 57 articles written both
        // ways, Crossref bare and PubMed punctuated. Adding them is now
        // only a decision and a merge, and it is the user's to make. So
        // both stay off, both spellings stay two names, and the off-list
        // counter keeps reporting them. See ELSEVIER.
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
        ] {
            let dois = [Doi::parse(doi).unwrap()];
            let scope = scoped(&dois);
            assert!(!is_elsevier(scope.registrant().unwrap()), "{doi}");
            let written = scope.name(punctuated).unwrap();
            let read = scope.name(bare).unwrap();
            assert_ne!(written, read, "{doi}: off the list, so two names");
            // Both still round-trip, punctuation and all, which is what
            // lets the registry keep the names it already sealed.
            for name in [&written, &read] {
                assert_eq!(Name::parse(&name.to_string()).unwrap(), *name);
            }
            assert!(pii_shaped(punctuated) && pii_shaped(bare), "{doi}");
        }
    }

    #[test]
    fn springers_prefix_is_left_off_and_keeps_being_counted() {
        // The same evidence in miniature — 23 PII-shaped values under
        // 10.1007 and 6 articles written both ways on the survey of
        // 2026-09-19, against 10.1038's 33 and 10.1136's 28, which are
        // off the list too. So the two spellings stay two names, the
        // punctuated one keeps the punctuation Elsevier's machinery
        // printed under Springer's DOI, and the counter keeps asking.
        let dois = [Doi::parse("10.1007/s11605-012-1859-9").unwrap()];
        let scope = scoped(&dois);
        assert!(!is_elsevier(scope.registrant().unwrap()));
        let punctuated = scope.name("S1091-255X(23)03813-1").unwrap();
        let bare = scope.name("S1091255X23038131").unwrap();
        assert_ne!(punctuated, bare, "not ours to merge");
        assert_eq!(punctuated.to_string(), "reg1007:S1091-255X(23)03813-1");
        assert_eq!(bare.to_string(), "reg1007:S1091255X23038131");
        assert!(pii_shaped("S1091-255X(23)03813-1"), "counted as off-list");
        assert!(pii_shaped("S1091255X23038131"), "counted as off-list");
    }

    #[test]
    fn the_list_is_in_order_and_holds_each_registrant_once() {
        // The doc asks for the digits in order; a duplicate or a value
        // out of place is a merge conflict resolved by hand going wrong.
        assert!(ELSEVIER.windows(2).all(|w| w[0] < w[1]), "{ELSEVIER:?}");
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
}
