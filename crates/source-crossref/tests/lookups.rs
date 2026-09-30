//! Real works looked up from the REST API on 2026-09-15 — the POC sample's
//! PubMed seed seen from Crossref — one per form the 1,000-work sample
//! never showed (plan §6, §9 M5).

use ingest::Sink;
use model::Doi;
use source_crossref::normalize::{LicenseStart, Reference, Work, normalize};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/fixtures/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[derive(Default)]
struct Collect<'n> {
    works: Vec<Work<'n>>,
    references: Vec<Reference<'n>>,
}

impl<'n> Sink<'n, source_crossref::Crossref> for Collect<'n> {
    fn work(&mut self, w: &Work<'n>) {
        self.works.push(w.clone());
    }
    fn reference(&mut self, _: &Doi<'n>, r: &Reference<'n>) {
        self.references.push(r.clone());
    }
    fn deletion(&mut self, _: &Doi<'n>) {}
}

fn run(name: &str) -> Collect<'static> {
    normalized(name, fixture(name))
}

/// A record as JSON, normalized.
fn normalized(name: &str, input: String) -> Collect<'static> {
    let input = Box::leak(input.into_boxed_str());
    let native: &'static source_crossref::Work<'static> =
        Box::leak(Box::new(serde_json::from_str(input).expect("parses")));
    let mut out = Collect::default();
    normalize(native, &mut out).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(out.works.len(), 1, "{name}");
    out
}

#[test]
fn a_typed_issn_with_a_bad_check_digit_is_a_claim() {
    let out = run("issn_bad_check_digit");
    let issns = &out.works[0].issns;
    assert!(!issns.is_empty());
    assert!(
        issns.iter().any(|t| t.issn.is_malformed()),
        "the source's check digit is wrong"
    );
}

#[test]
fn an_affiliation_may_carry_an_isni_beside_its_ror() {
    let out = run("affiliation_isni");
    let with_isni: Vec<_> = out.works[0]
        .authors
        .iter()
        .flat_map(|a| &a.affiliations)
        .filter(|af| af.isni.is_some())
        .collect();
    assert!(!with_isni.is_empty());
    assert_eq!(
        with_isni[0]
            .isni
            .and_then(|i| i.id.valid().map(ToString::to_string))
            .as_deref(),
        Some("0000000404370279")
    );
    assert!(
        with_isni[0].ror.is_some_and(|r| r.id.valid().is_some()),
        "beside the ROR"
    );
}

/// 10.3389/fmicb.2019.02678, looked up 2026-09-26: Frontiers deposited
/// `start_date="0001-01-01"`, a date library's "no date", and the API
/// writes it in all three forms. The one such start the survey of the
/// March 2026 file found; kept as written beside why, and the record
/// normalizes.
#[test]
fn a_license_starting_in_year_one_is_unreadable_and_kept() {
    let out = run("license_start_year_one");
    assert_eq!(
        out.works[0].licenses[0].start,
        LicenseStart::Unreadable {
            raw: "0001-01-01T00:00:00Z",
            error: model::DateError::YearRange
        }
    );
}

/// What the merge asks of a record the identity pass reads (merge plan
/// §11 item 28): whether it is a container's, the aliases it gives, and
/// the names it says it corrects.
fn asked(name: &str) -> (bool, Vec<String>, Vec<String>) {
    use merge::Contributor;
    let out = run(name);
    let work = &out.works[0];
    let container = source_crossref::Crossref::container(work);
    let mut aliases = Vec::new();
    source_crossref::Crossref::names(work, &mut |a| {
        if a.field == merge::ALIASES {
            aliases.push(a.name.to_string());
        }
    });
    let mut corrects = Vec::new();
    source_crossref::view::relations(work, &model::Table::default(), &mut |r| {
        if r.kind.corrects() {
            corrects.extend(r.target.map(|t| t.to_string()));
        }
    })
    .unwrap();
    (container, aliases, corrects)
}

/// The three records the survey of the whole March 2026 file stopped on
/// (homelab `ol-rht-merge-survey-v8`, 2026-09-27), as the REST API gave
/// them the same day: an article giving its volume's DOI, cut short, as
/// an alias — 36 of that volume's articles do.
#[test]
fn an_article_gives_its_volumes_doi_as_an_alias() {
    let (container, aliases, corrects) = asked("article_aliases_its_volume");
    assert!(!container);
    assert_eq!(aliases, ["doi:10.1089/acm.2004.10"]);
    assert!(corrects.is_empty());
}

/// A correction that gives the abstracts it corrects as its aliases, and
/// updates one of them.
#[test]
fn a_correction_gives_what_it_corrects_as_its_aliases() {
    let (container, aliases, corrects) = asked("correction_aliases_what_it_corrects");
    assert!(!container);
    assert_eq!(aliases.len(), 102);
    assert_eq!(corrects, ["doi:10.1161/svin.01.suppl_1.000048"]);
    assert!(aliases.contains(&corrects[0]));
}

/// That correction's deposit is on Crossref's list of deposits whose
/// aliases are what they correct (`source_crossref::listed_corrections`),
/// since nothing PubMed's 2026 baseline says settles it; the same record
/// deposited again is not, and is judged on what it says.
#[test]
fn a_listed_corrections_aliases_are_what_it_corrects_and_a_later_deposits_are_not() {
    use merge::Contributor;
    let name = "correction_aliases_what_it_corrects";
    let listed = run(name);
    assert!(source_crossref::Crossref::corrects_its_aliases(
        &listed.works[0]
    ));
    let again = fixture(name).replace(
        r#"[[2024, 5, 13]], "date-time": "2024-05-13T11:40:20Z", "timestamp": 1715600420000"#,
        r#"[[2026, 9, 27]], "date-time": "2026-09-27T00:00:00Z", "timestamp": 1790467200000"#,
    );
    assert_ne!(again, fixture(name));
    let again = normalized(name, again);
    assert!(!source_crossref::Crossref::corrects_its_aliases(
        &again.works[0]
    ));
}

/// A journal's record, which gives its articles' old DOIs as its aliases:
/// a container's.
#[test]
fn a_journals_record_is_a_containers() {
    let (container, aliases, corrects) = asked("journal_aliases_its_articles");
    assert!(container);
    assert_eq!(aliases.len(), 655);
    assert!(aliases.contains(&"doi:10.1039/b418513g".to_owned()));
    assert!(corrects.is_empty());
}
