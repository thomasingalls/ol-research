//! Real works from the API sample, one per shape that mattered when the
//! struct was written; each is also the first regression fixture for that
//! shape (plan §6).

use source_crossref::Work;
use source_crossref::vocab::{UpdateType, WorkType};

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

#[test]
fn journal_article_with_everything() {
    let input = fixture("journal_article_full");
    let w = work(&input);
    assert_eq!(w.r#type, WorkType::JournalArticle);
    assert!(!w.reference.as_ref().unwrap().is_empty());
    assert!(w.r#abstract.as_ref().unwrap().contains("<jats:"));
    assert!(
        w.author
            .as_ref()
            .unwrap()
            .iter()
            .all(|a| a.family.is_some() || a.name.is_some())
    );
    assert!(w.issued.date_parts.year.is_some());
    assert_eq!(w.url, format!("https://doi.org/{}", w.doi));
}

#[test]
fn the_grant_has_a_project_and_no_counts() {
    let input = fixture("grant_project");
    let w = work(&input);
    assert_eq!(w.r#type, WorkType::Grant);
    assert!(w.project.is_some());
    assert!(w.reference_count.is_none());
}

#[test]
fn issued_can_be_null() {
    let input = fixture("issued_null_component");
    let w = work(&input);
    assert!(w.issued.date_parts.is_null());
}

#[test]
fn contributor_roles_can_be_null() {
    let input = fixture("contributor_null_role");
    let w = work(&input);
    let c = &w.contributor.as_ref().unwrap()[0];
    assert!(c.role.as_ref().unwrap().iter().all(Option::is_none));
}

#[test]
fn retractions_arrive_as_updated_by() {
    let input = fixture("updated_by_retraction");
    let w = work(&input);
    assert!(
        w.updated_by
            .as_ref()
            .unwrap()
            .iter()
            .any(|u| u.r#type == UpdateType::Retraction)
    );
}

#[test]
fn relations_are_keyed_by_type() {
    let input = fixture("relation_has_preprint");
    let w = work(&input);
    assert!(w.relation.as_ref().unwrap().contains_key("has-preprint"));
}

#[test]
fn every_fixture_clones_equal() {
    for name in [
        "journal_article_full",
        "grant_project",
        "issued_null_component",
        "contributor_null_role",
        "updated_by_retraction",
        "relation_has_preprint",
        "book_chapter_isbn",
    ] {
        let input = fixture(name);
        let w = work(&input);
        assert_eq!(w.clone(), w, "{name}");
    }
}

#[test]
fn a_new_key_anywhere_fails_with_its_name() {
    let input = fixture("journal_article_full").replacen("\"publisher\":", "\"publisher_\":", 1);
    let err = serde_json::from_str::<Work<'_>>(&input).unwrap_err();
    assert!(
        err.to_string().contains("unknown field `publisher_`"),
        "{err}"
    );

    // A value outside a closed vocabulary, whichever value the fixture has.
    let input = fixture("journal_article_full").replacen(
        "\"doi-asserted-by\":\"",
        "\"doi-asserted-by\":\"X",
        1,
    );
    assert!(
        input.contains("\"doi-asserted-by\":\"X"),
        "fixture asserts a DOI"
    );
    let err = serde_json::from_str::<Work<'_>>(&input).unwrap_err();
    assert!(err.to_string().contains("unknown variant `X"), "{err}");
}
