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
    let input = Box::leak(fixture(name).into_boxed_str());
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
        with_isni[0].isni.map(|i| i.id.to_string()).as_deref(),
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
