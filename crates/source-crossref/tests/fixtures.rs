//! Real works from the API sample, one per shape that mattered when the
//! struct was written; each is also the first regression fixture for that
//! shape (plan §6).

use source_crossref::Work;
use source_crossref::vocab::{UpdateWord, WorkType};

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
    assert_eq!(w.r#type, Some(WorkType::JournalArticle));
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
    assert_eq!(w.r#type, Some(WorkType::Grant));
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
            .any(|u| u.r#type == UpdateWord::Retraction)
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

mod normalized {
    use super::{fixture, work};
    use ingest::Sink;
    use model::{Claim, Doi};
    use source_crossref::Crossref;
    use source_crossref::normalize::{AuthorName, Reference, Work, normalize};
    use source_crossref::vocab::{UpdateType, WorkType};

    #[derive(Default)]
    struct Collect<'n> {
        works: Vec<Work<'n>>,
        references: Vec<(String, Reference<'n>)>,
    }

    impl<'n> Sink<'n, Crossref> for Collect<'n> {
        fn work(&mut self, w: &Work<'n>) {
            self.works.push(w.clone());
        }
        fn reference(&mut self, from: &Doi<'n>, r: &Reference<'n>) {
            self.references.push((from.to_string(), r.clone()));
        }
        fn deletion(&mut self, _: &Doi<'n>) {}
    }

    fn run(name: &str) -> (String, Collect<'static>) {
        let input = Box::leak(fixture(name).into_boxed_str());
        let native = Box::leak(Box::new(work(input)));
        let mut out = Collect::default();
        normalize(native, &mut out).expect("normalizes");
        (input.to_owned(), out)
    }

    #[test]
    fn journal_article_normalizes_with_edges() {
        let (_, out) = run("journal_article_full");
        assert_eq!(out.works.len(), 1);
        let w = &out.works[0];
        assert_eq!(w.kind, Some(WorkType::JournalArticle));
        assert_eq!(w.doi.as_str(), "10.1002/jha2.1033");
        assert!(
            w.created.epoch_seconds() > 1_000_000_000,
            "a real 21st-century instant"
        );
        assert!(w.issued.is_some());
        assert!(!w.issns.is_empty());
        assert!(
            w.authors
                .iter()
                .all(|a| matches!(a.name, AuthorName::Person { .. }))
        );
        assert!(w.title.is_some());
        assert!(w.r#abstract.is_some());
        assert_eq!(
            Some(out.references.len()),
            w.references_count.map(|n| n as usize)
        );
        assert!(
            out.references
                .iter()
                .all(|(from, _)| from == "10.1002/jha2.1033")
        );
        assert!(
            out.references
                .iter()
                .any(|(_, r)| matches!(r.doi, Some(Claim::Valid(_))))
        );
    }

    #[test]
    fn grant_and_null_issued() {
        let (_, out) = run("grant_project");
        assert_eq!(out.works[0].kind, Some(WorkType::Grant));
        assert_eq!(out.works[0].references_count, None);
        assert!(out.references.is_empty());
        let (_, out) = run("issued_null_component");
        assert_eq!(out.works[0].issued, None);
    }

    #[test]
    fn updates_and_relations() {
        let (_, out) = run("updated_by_retraction");
        assert!(
            out.works[0]
                .updated_by
                .iter()
                .any(|u| u.kind == UpdateType::Retraction)
        );
        let (_, out) = run("relation_has_preprint");
        assert!(
            out.works[0]
                .relations
                .iter()
                .any(|r| r.kind == "has-preprint")
        );
    }

    #[test]
    fn invariants_fail_fast_with_the_doi() {
        for (from, to, field) in [
            (
                "\"URL\":\"https://doi.org/10.1002/jha2.1033\"",
                "\"URL\":\"https://doi.org/10.1002/jha2.1034\"",
                "URL",
            ),
            ("\"prefix\":\"10.1002\"", "\"prefix\":\"10.0002\"", "prefix"),
        ] {
            let input = fixture("journal_article_full").replacen(from, to, 1);
            assert!(input.contains(to), "{field}: edit applied");
            let native = work(&input);
            let err = normalize(&native, &mut Collect::default()).unwrap_err();
            assert_eq!(err.field, field, "{err}");
            assert_eq!(err.doi.as_deref(), Some("10.1002/jha2.1033"));
        }
    }

    #[test]
    fn a_timestamp_off_by_a_second_is_caught() {
        let input = fixture("journal_article_full");
        let start = input.find("\"created\":{").unwrap();
        let ts_at = input[start..].find("\"timestamp\":").unwrap() + start + "\"timestamp\":".len();
        let end = input[ts_at..].find(|c: char| !c.is_ascii_digit()).unwrap() + ts_at;
        let bumped: i64 = input[ts_at..end].parse::<i64>().unwrap() + 1000;
        let input = format!("{}{}{}", &input[..ts_at], bumped, &input[end..]);
        let native = work(&input);
        let err = normalize(&native, &mut Collect::default()).unwrap_err();
        assert_eq!(err.field, "created", "{err}");
        assert!(err.to_string().contains("timestamp disagrees"), "{err}");
    }

    #[test]
    fn the_binary_layout_roundtrips_the_fixtures() {
        use ingest::{Binary, Kind, Reader};
        let prefix = std::env::temp_dir().join(format!("ol-rht-crossref-{}", std::process::id()));
        let mut binary = Binary::new("crossref", model::VERSION, {
            let prefix = prefix.clone();
            move |_: usize, _: &std::path::Path| prefix.clone()
        });
        binary
            .open_file(std::path::Path::new("fixtures"), 0)
            .unwrap();
        let inputs: Vec<String> = ["journal_article_full", "grant_project"]
            .iter()
            .map(|n| fixture(n))
            .collect();
        let natives: Vec<source_crossref::Work<'_>> = inputs.iter().map(|i| work(i)).collect();
        let mut collect = Collect::default();
        for native in &natives {
            normalize(native, &mut (&mut collect, &mut binary)).expect("normalizes");
        }
        binary.close_file().unwrap();
        let written = binary.finish().unwrap();
        assert_eq!(written.len(), 1);
        let written = &written[0];
        assert_eq!(written.records[0], 2);
        let works = Reader::open(&Kind::Works.path(&prefix)).unwrap();
        let back: Vec<Work<'_>> = works.records().map(Result::unwrap).collect();
        // Interned strings come back as symbols; through the set's table
        // they serialize to the same text.
        assert!(works.table().is_some_and(|t| !t.is_empty()));
        assert_eq!(
            works.with_table(|| serde_json::to_string(&back).unwrap()),
            serde_json::to_string(&collect.works).unwrap()
        );
        let refs = Reader::open(&Kind::References.path(&prefix)).unwrap();
        let back: Vec<(Doi<'_>, Reference<'_>)> = refs.records().map(Result::unwrap).collect();
        assert_eq!(back.len(), collect.references.len());
        for ((from, r), (from2, r2)) in back.iter().zip(&collect.references) {
            assert_eq!(from.as_str(), from2);
            assert_eq!(r, r2);
        }
        for kind in Kind::NORMALIZED {
            std::fs::remove_file(kind.path(&prefix)).unwrap();
        }
        // Every source names its records, so every set has an index.
        std::fs::remove_file(ingest::binary::Index::path(&prefix)).unwrap();
    }
}
