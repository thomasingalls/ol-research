//! Records from Crossref's March 2026 public data file, each the line as
//! shipped, pulled off the box where the survey of the whole file stopped
//! (homelab `ol-rht-dump-crossref-v1`, 2026-09-26): one per shape the
//! survey found, and the first regression fixture for it (plan §6).

use ingest::Sink;
use model::{Claim, Doi, RorError};
use source_crossref::normalize::{Institution, Reference, Work, normalize};
use source_crossref::vocab::AssertedBy;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/fixtures/march2026/{name}.json",
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

fn run(name: &str) -> Work<'static> {
    let input = Box::leak(fixture(name).into_boxed_str());
    let native: &'static source_crossref::Work<'static> =
        Box::leak(Box::new(serde_json::from_str(input).expect("parses")));
    let mut out = Collect::default();
    normalize(native, &mut out).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(out.works.len(), 1, "{name}");
    out.works.remove(0)
}

fn texts<'a>(list: &'a [model::Interned<'a>]) -> Vec<&'a str> {
    list.iter().map(|t| t.as_text().unwrap()).collect()
}

/// 10.17077/pp.006703, shard 6896 line 3174: the University of Iowa's
/// repository, with a ROR, an ISNI and a Wikidata item on the author's
/// affiliation and again on the work's own institution.
#[test]
fn an_institution_carries_ror_isni_and_wikidata_with_who_asserted_them() {
    let w = run("institution_wikidata");
    let affiliation = &w.authors[0].affiliations[0];
    let own = &w.institutions[0];
    for i in [affiliation, own] {
        assert_eq!(i.name.and_then(|n| n.as_text()), Some("University of Iowa"));
        let ror = i.ror.unwrap();
        assert_eq!(
            (ror.id.valid().map(ToString::to_string), ror.by),
            (Some("036jqmy94".to_owned()), AssertedBy::Publisher)
        );
        assert_eq!(
            i.isni.map(|a| (a.id.to_string(), a.by)),
            Some(("0000000419368294".to_owned(), AssertedBy::Publisher))
        );
        assert_eq!(
            i.wikidata.map(|a| (a.id.to_string(), a.by)),
            Some(("Q182973".to_owned(), AssertedBy::Publisher))
        );
    }
    assert_eq!(texts(&own.departments), ["Physics and Astronomy"]);
    assert_eq!(texts(&own.places), ["Iowa City, IA, United States"]);
    assert!(affiliation.places.is_empty() && affiliation.acronyms.is_empty());
}

/// 10.26419/res.00973.001, shard 2684 line 2758: AARP's research
/// report, with an acronym on the author's affiliation, where the parser
/// had accepted one only on a work's own institution.
#[test]
fn an_affiliation_keeps_its_acronym_department_and_place() {
    let w = run("affiliation_acronym");
    let a: &Institution<'_> = &w.authors[0].affiliations[0];
    assert_eq!(texts(&a.acronyms), ["AARP"]);
    assert_eq!(texts(&a.departments), ["Research"]);
    assert_eq!(texts(&a.places), ["Washington, DC"]);
    assert_eq!(a.name.and_then(|n| n.as_text()), Some("AARP"));
    assert!(a.ror.is_none() && a.isni.is_none() && a.wikidata.is_none());
    assert_eq!(w.institutions.len(), 1, "the work's own institution too");
}

/// 10.25189/2675-4916.2023.v4.n1.id679, shard 1962 line 3231: the
/// Universidade Federal Fluminense's ROR written as `03npvkt06`, whose
/// checksum would be `82`; its ROR is `02rjhbb08`. Kept as written beside
/// why, and the record normalizes.
#[test]
fn a_ror_whose_checksum_fails_is_a_claim() {
    let w = run("affiliation_ror_bad_checksum");
    let rors: Vec<_> = w
        .authors
        .iter()
        .flat_map(|a| &a.affiliations)
        .filter_map(|a| a.ror)
        .collect();
    assert_eq!(rors.len(), 2, "two authors, one bad ROR each");
    for r in rors {
        assert_eq!(
            r.id,
            Claim::Malformed {
                raw: "https://ror.org/03npvkt06",
                error: RorError::Checksum
            }
        );
    }
}
