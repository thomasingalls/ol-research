//! Real records looked up from NCBI's `efetch` on 2026-09-15 — the cited
//! works of the POC sample's one-hop expansion — one per form the 2026
//! baseline files never showed (plan §6, §9 M5). Each is the regression
//! fixture for that form.
//!
//! Four more were looked up on 2026-09-23, named by the first survey of the
//! whole baseline as the records behind two of its signatures: the shapes
//! are too rare for the two files on this machine to hold one.

use ingest::Sink;
use model::{Claim, PartialDate, Pmid, Season};
use source_pubmed::attr::{AffiliationIdSource, NoteOwner, OtherIdSource};
use source_pubmed::normalize::{Citation, Published, Reference, normalize};
use source_pubmed::record::Record;
use source_pubmed::{ArticleSet, Pubmed};

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/{name}.xml", env!("CARGO_MANIFEST_DIR"));
    let body = std::fs::read_to_string(&path).expect("fixture exists");
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<PubmedArticleSet>\n{body}</PubmedArticleSet>\n"
    )
}

#[derive(Default)]
struct Collect<'n> {
    citations: Vec<Citation<'n>>,
    references: Vec<Reference<'n>>,
}

impl<'n> Sink<'n, Pubmed> for Collect<'n> {
    fn work(&mut self, c: &Citation<'n>) {
        self.citations.push(c.clone());
    }
    fn reference(&mut self, _: &Pmid, reference: &Reference<'n>) {
        self.references.push(reference.clone());
    }
    fn deletion(&mut self, _: &Pmid) {}
}

/// The one record of a fixture, normalized.
fn run(name: &str) -> Collect<'static> {
    let input = Box::leak(fixture(name).into_boxed_str());
    let mut set = ArticleSet::new(input);
    let Some(Record::Article(article)) = set.next_record().expect("parses") else {
        panic!("{name}: not an article")
    };
    let article = Box::leak(Box::new(article));
    let mut out = Collect::default();
    normalize(article, &mut out).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(out.citations.len(), 1, "{name}");
    out
}

#[test]
fn a_reference_id_may_be_typed_pmc_as_well_as_pmcid() {
    let out = run("ref_pmc_id");
    assert!(
        out.references
            .iter()
            .any(|r| matches!(r.pmc, Some(Claim::Valid(_))))
    );
}

#[test]
fn older_records_count_their_references_even_to_zero() {
    assert_eq!(
        run("number_of_references").citations[0].number_of_references,
        Some(412)
    );
    assert_eq!(
        run("zero_references").citations[0].number_of_references,
        Some(0)
    );
}

#[test]
fn a_season_may_be_a_month_range_with_a_hyphen_or_a_slash_or_autumn() {
    for name in ["season_month_range", "season_slash"] {
        assert!(
            matches!(
                run(name).citations[0].dates.published,
                Published::Read(PartialDate::Span(_))
            ),
            "{name}"
        );
    }
    assert!(matches!(
        run("season_autumn").citations[0].dates.published,
        Published::Read(PartialDate::YearSeason(_, Season::Fall))
    ));
}

#[test]
fn elocation_ids_may_repeat_a_type() {
    let ids = &run("two_piis").citations[0].ids;
    assert_eq!(ids.elocation_pii.len(), 2);
    assert_eq!(ids.elocation_doi.len(), 1);
    let ids = &run("two_dois").citations[0].ids;
    assert_eq!(ids.elocation_doi.len(), 2);
    assert!(ids.elocation_doi.iter().any(Claim::is_malformed));
    assert!(ids.elocation_doi.iter().any(|d| d.valid().is_some()));
}

#[test]
fn the_rarer_identifiers_are_kept_by_kind() {
    let c = &run("other_id").citations[0];
    assert_eq!(c.other_ids.len(), 1);
    assert_eq!(c.other_ids[0].source, OtherIdSource::Nlm);
    assert_eq!(c.other_ids[0].value, "UKMS5225");
    assert_eq!(run("medline_id").citations[0].ids.medline, Some("9509184"));
    assert!(run("sici").citations[0].ids.sici.is_some());
    assert_eq!(run("mid_cams").citations[0].ids.mid, Some("CAMS6040"));
}

#[test]
fn the_rarer_elements_are_kept() {
    assert_eq!(&*run("gene_symbols").citations[0].gene_symbols, ["IifA"]);
    let c = &run("personal_name_subject").citations[0];
    assert_eq!(c.personal_name_subjects.len(), 1);
    assert_eq!(c.personal_name_subjects[0].last, "Vasquez");
    let c = &run("general_note").citations[0];
    assert_eq!(c.general_notes.len(), 1);
    assert_eq!(c.general_notes[0].owner, NoteOwner::Pip);
    assert!(c.general_notes[0].text.starts_with("TJ:"));
}

#[test]
fn a_language_that_is_not_a_code_is_a_claim() {
    let c = &run("other_abstract_lang_sp").citations[0];
    assert!(c.other_abstracts.iter().any(|o| o.language.is_malformed()));
}

/// The one `Language` in the 2026 baseline that is not an ISO 639-2 code:
/// `crg`, beside `eng`, on an English paper. The record normalizes, and
/// says only what it is in.
#[test]
fn a_language_that_is_no_language_at_all_is_dropped() {
    let c = &run("language_crg").citations[0];
    assert_eq!(c.pmid.get(), 40_316_125);
    assert_eq!(
        c.languages
            .iter()
            .map(|l| l.bibliographic())
            .collect::<Vec<_>>(),
        ["eng"]
    );
}

/// The three `AffiliationInfo/Identifier@Source` values that name no
/// organization registry. Each record normalizes, keeps its affiliation
/// text, and carries no identifier it cannot stand behind.
#[test]
fn an_affiliation_identifier_that_names_no_organization_is_dropped() {
    // A person's ORCID, filed in the affiliation envelope: the author has
    // no identifier of its own, and does not gain one from this.
    let c = &run("affiliation_id_orcid").citations[0];
    let a = &c.authors[0];
    assert!(a.orcid.is_none());
    assert!(a.affiliations[0].text.contains("Johns Hopkins"));
    assert!(a.affiliations[0].ids.is_empty());

    // Elsevier's own key for the affiliation line.
    let c = &run("affiliation_id_es_uid").citations[0];
    assert!(c.authors[0].affiliations[0].text.contains("Vanderbilt"));
    assert!(c.authors[0].affiliations[0].ids.is_empty());

    // A Ringgold identifier doubled, on a record whose other affiliations
    // carry a plain one: the plain ones survive, the doubled ones do not.
    let c = &run("affiliation_id_doubled").citations[0];
    let ids: Vec<_> = c
        .authors
        .iter()
        .flat_map(|a| a.affiliations.iter())
        .flat_map(|af| af.ids.iter())
        .collect();
    assert!(!ids.is_empty());
    assert!(
        ids.iter()
            .all(|id| id.source == AffiliationIdSource::Ringgold)
    );
    assert!(ids.iter().all(|id| id.value == "7890"));
}

/// A source outside both the four registries and the read warts is still a
/// failure: the vocabulary is closed, and the survey's samples are a sample.
#[test]
fn an_unread_affiliation_identifier_source_still_stops_the_run() {
    let input = fixture("affiliation_id_es_uid").replace("ES-UID", "WIKIDATA");
    let mut set = ArticleSet::new(&input);
    let Some(Record::Article(article)) = set.next_record().expect("parses") else {
        panic!("not an article")
    };
    let err = normalize(&article, &mut Collect::default()).unwrap_err();
    assert!(
        err.to_string()
            .contains("not in the vocabulary of affiliation identifier sources"),
        "{err}"
    );
}
