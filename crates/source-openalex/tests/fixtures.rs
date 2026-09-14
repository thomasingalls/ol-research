//! Real works from the part file, one per shape that mattered when the
//! struct was written; each is also the first regression fixture for that
//! shape (plan §6).

use source_openalex::vocab::{OaStatus, Version, WorkType};
use source_openalex::{Work, jsonl};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/fixtures/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn work(input: &str) -> Work<'_> {
    serde_json::from_str(input).expect("parses")
}

const ALL: [&str; 14] = [
    "smallest",
    "pubmed_with_mesh",
    "dissertation_repo_pdf",
    "awards_and_funders",
    "apc_paid",
    "null_author_id_no_abstract",
    "closed_no_oa_location",
    "empty_doi",
    "bad_issn",
    "language_sh",
    "language_ceb",
    "mag_differs",
    "lone_last_author",
    "self_reference",
];

#[test]
fn pubmed_sourced_work_has_mesh_and_a_pmid() {
    let input = fixture("pubmed_with_mesh");
    let w = work(&input);
    assert!(!w.mesh.is_empty());
    assert!(
        w.ids
            .pmid
            .as_deref()
            .unwrap()
            .starts_with("https://pubmed.ncbi.nlm.nih.gov/")
    );
    assert_eq!(w.ids.openalex, w.id);
}

#[test]
fn a_dissertation_in_a_repository_with_a_pdf() {
    let input = fixture("dissertation_repo_pdf");
    let w = work(&input);
    assert_eq!(w.r#type, WorkType::Dissertation);
    assert!(w.best_oa_location.pdf_url.is_some());
    assert_eq!(w.best_oa_location.is_oa, Some(true));
    assert!(w.locations.iter().any(|l| l.version == Some(Version::Submitted) || l.version == Some(Version::Published)));
}

#[test]
fn closed_works_have_an_object_of_nulls_for_best_oa_location() {
    let input = fixture("closed_no_oa_location");
    let w = work(&input);
    assert_eq!(w.open_access.oa_status, OaStatus::Closed);
    assert_eq!(w.best_oa_location.id, None);
    assert_eq!(w.best_oa_location.is_oa, None);
    assert!(!w.referenced_works.is_empty());
    assert_eq!(w.referenced_works.len(), w.referenced_works_count as usize);
}

#[test]
fn awards_funders_and_the_paid_apc() {
    let input = fixture("awards_and_funders");
    let w = work(&input);
    assert!(!w.awards.is_empty() && !w.funders.is_empty());
    let input = fixture("apc_paid");
    let w = work(&input);
    assert!(w.apc_paid.is_some());
}

#[test]
fn abstracts_are_raw_slices_or_absent() {
    let input = fixture("null_author_id_no_abstract");
    let w = work(&input);
    assert!(w.abstract_inverted_index.is_none());
    assert!(w.authorships.iter().any(|a| a.author.id.is_none()));
    let inputs: Vec<String> = ALL.iter().map(|n| fixture(n)).collect();
    let with_abstract = inputs
        .iter()
        .map(|i| work(i))
        .find_map(|w| w.abstract_inverted_index)
        .expect("some fixture carries an abstract");
    assert!(with_abstract.as_str().starts_with('{'));
    assert!(with_abstract.as_str().contains('['), "word -> positions");
}

#[test]
fn every_fixture_clones_equal_and_reads_as_jsonl() {
    let mut all = String::new();
    for name in ALL {
        let input = fixture(name);
        let w = work(&input);
        assert_eq!(w.clone(), w, "{name}");
        all.push_str(&input);
    }
    assert_eq!(jsonl(&all).count(), ALL.len());
    assert!(jsonl(&all).all(|w| w.is_ok()));
}

#[test]
fn a_new_key_anywhere_fails_with_line_and_id() {
    let input = fixture("smallest").replacen("\"is_xpac\":", "\"is_xpac2\":", 1);
    assert!(input.contains("is_xpac2"));
    let err = jsonl(&input).next().unwrap().unwrap_err();
    assert_eq!(err.line, 1);
    assert!(
        err.id
            .as_deref()
            .unwrap()
            .starts_with("https://openalex.org/W")
    );
    assert!(
        err.to_string().contains("unknown field `is_xpac2`"),
        "{err}"
    );
    let input = fixture("smallest").replacen("\"oa_status\":\"", "\"oa_status\":\"x", 1);
    assert!(input.contains("\"oa_status\":\"x"));
    let err = jsonl(&input).next().unwrap().unwrap_err();
    assert!(err.to_string().contains("unknown variant `x"), "{err}");
}
