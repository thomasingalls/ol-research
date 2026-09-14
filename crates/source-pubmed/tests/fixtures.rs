//! Real records from `pubmed26n1334.xml.gz`, one per shape that mattered
//! when the parser was written. Each is also the first regression fixture
//! for that shape (plan §6).

use source_pubmed::attr::{IdType, PubStatus, RefType, Status};
use source_pubmed::record::{AuthorName, PubDate, Record};
use source_pubmed::{ArticleSet, ErrorKind};

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/{name}.xml", env!("CARGO_MANIFEST_DIR"));
    let body = std::fs::read_to_string(&path).expect("fixture exists");
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<PubmedArticleSet>\n{body}</PubmedArticleSet>\n"
    )
}

fn only_article(input: &str) -> source_pubmed::record::PubmedArticle<'_> {
    let mut set = ArticleSet::new(input);
    let Some(Record::Article(article)) = set.next_record().expect("parses") else {
        panic!("expected one article")
    };
    assert!(set.next_record().expect("clean end").is_none());
    article
}

#[test]
fn shortest_record_parses_completely() {
    let input = fixture("shortest");
    let a = only_article(&input);
    let c = &a.medline_citation;
    assert_eq!(&*c.pmid.value, "41609757");
    assert_eq!(&*c.pmid.version, "1");
    assert_eq!(c.status, Status::Publisher);
    assert_eq!(c.indexing_method.as_deref(), Some("Automated"));
    assert_eq!(
        c.article.article_title.content.as_str(),
        "Glucose as a Food."
    );
    assert_eq!(c.article.journal.title.as_deref(), Some("JAMA"));
    let eloc = &c.article.elocation_ids[0];
    assert_eq!(&*eloc.value, "10.1001/jama.2025.15703");
    assert!(
        matches!(c.article.journal.journal_issue.pub_date, PubDate::Date { ref month, .. } if month.as_deref() == Some("Jan"))
    );
    let d = a.pubmed_data.as_ref().unwrap();
    assert_eq!(&*d.publication_status, "aheadofprint");
    assert_eq!(d.history[0].pub_status, PubStatus::Entrez);
    assert_eq!(d.history[0].hour.as_deref(), Some("12"));
    assert_eq!(d.article_ids.len(), 3);
    assert_eq!(d.article_ids[1].id_type, IdType::Pii);
}

#[test]
fn medline_date_takes_the_free_text_branch() {
    let input = fixture("medline_date");
    let a = only_article(&input);
    assert!(matches!(
        a.medline_citation.article.journal.journal_issue.pub_date,
        PubDate::Medline(_)
    ));
}

#[test]
fn collective_names_take_the_collective_branch() {
    let input = fixture("collective_name");
    let a = only_article(&input);
    let authors = &a
        .medline_citation
        .article
        .author_list
        .as_ref()
        .unwrap()
        .authors;
    assert!(
        authors
            .iter()
            .any(|x| matches!(x.name, AuthorName::Collective(_)))
    );
}

#[test]
fn references_carry_pmcid_and_corrections_carry_ref_types() {
    let input = fixture("refs_pmcid_corrections");
    let a = only_article(&input);
    let refs = &a.pubmed_data.as_ref().unwrap().reference_lists;
    assert!(!refs.is_empty());
    let ids: Vec<IdType> = refs
        .iter()
        .flat_map(|l| &l.references)
        .flat_map(|r| &r.article_ids)
        .map(|id| id.id_type)
        .collect();
    assert!(ids.contains(&IdType::Pmcid));
    assert!(ids.contains(&IdType::Pubmed));
    let cc = &a.medline_citation.comments_corrections;
    assert!(!cc.is_empty());
    assert!(RefType::TOKENS.contains(&cc[0].ref_type.token()));
}

#[test]
fn mathml_stays_raw_inside_abstracts() {
    let input = fixture("mathml_refs_mesh");
    let a = only_article(&input);
    let abs = a.medline_citation.article.r#abstract.as_ref().unwrap();
    assert!(
        abs.texts
            .iter()
            .any(|t| t.content.as_str().contains("<mml:math"))
    );
    assert!(!a.medline_citation.mesh_headings.is_empty());
}

#[test]
fn every_fixture_round_trips_through_clone_and_eq() {
    for name in [
        "shortest",
        "medline_date",
        "collective_name",
        "refs_pmcid_corrections",
        "mathml_refs_mesh",
    ] {
        let input = fixture(name);
        let a = only_article(&input);
        assert_eq!(a.clone(), a, "{name}");
    }
}

#[test]
fn structural_disagreements_fail_with_location() {
    let input = fixture("shortest");
    // An element the DTD does not allow inside MedlineCitation.
    let bad = input.replace("<CitationSubset>IM</CitationSubset>", "<Mood>IM</Mood>");
    let err = ArticleSet::new(&bad).next_record().unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::UnexpectedElement(ref n) if n == "Mood"),
        "{err}"
    );
    assert_eq!(err.path, "PubmedArticleSet/PubmedArticle/MedlineCitation");
    assert_eq!(err.pmid.as_deref(), Some("41609757"));

    // A value outside an attribute's vocabulary.
    let bad = input.replace("Status=\"Publisher\"", "Status=\"publisher\"");
    let err = ArticleSet::new(&bad).next_record().unwrap_err();
    assert!(
        matches!(
            err.kind,
            ErrorKind::InvalidAttributeValue { name: "Status", .. }
        ),
        "{err}"
    );

    // A missing required element.
    let bad = input.replace("<PublicationStatus>aheadofprint</PublicationStatus>", "");
    let err = ArticleSet::new(&bad).next_record().unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::MissingElement("PublicationStatus")),
        "{err}"
    );

    // A duplicated singleton.
    let bad = input.replace(
        "<Title>JAMA</Title>",
        "<Title>JAMA</Title><Title>JAMA</Title>",
    );
    let err = ArticleSet::new(&bad).next_record().unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::DuplicateElement("Title")),
        "{err}"
    );

    // An attribute the DTD does not declare.
    let bad = input.replace("<Journal>", "<Journal Kind=\"x\">");
    let err = ArticleSet::new(&bad).next_record().unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::UnexpectedAttribute(ref n) if n == "Kind"),
        "{err}"
    );

    // Text where only elements may appear.
    let bad = input.replace("<Journal>", "<Journal>stray");
    let err = ArticleSet::new(&bad).next_record().unwrap_err();
    assert!(
        matches!(err.kind, ErrorKind::UnexpectedText(ref t) if t == "stray"),
        "{err}"
    );

    // An undefined entity in a leaf.
    let bad = input.replace("<Title>JAMA</Title>", "<Title>J&nbsp;AMA</Title>");
    let err = ArticleSet::new(&bad).next_record().unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Escape(_)), "{err}");
}

mod normalized {
    use super::{fixture, only_article};
    use model::{Claim, Month, PartialDate, Pmid};
    use source_pubmed::attr::RefType;
    use source_pubmed::normalize::{Citation, Emit, Reference, normalize};

    #[derive(Default)]
    struct Collect<'n> {
        citations: Vec<Citation<'n>>,
        references: Vec<Reference<'n>>,
    }

    impl<'n> Emit<'n> for Collect<'n> {
        fn citation(&mut self, citation: &Citation<'n>) {
            self.citations.push(citation.clone());
        }
        fn reference(&mut self, reference: &Reference<'n>) {
            self.references.push(reference.clone());
        }
    }

    #[test]
    fn shortest_record_normalizes() {
        let input = fixture("shortest");
        let article = only_article(&input);
        let mut out = Collect::default();
        normalize(&article, &mut out).expect("normalizes");
        assert_eq!(out.citations.len(), 1);
        let c = &out.citations[0];
        assert_eq!(c.pmid, Pmid::new(41_609_757).unwrap());
        assert_eq!(c.version, 1);
        let doi = c.doi.as_ref().unwrap().valid().unwrap();
        assert_eq!(doi.as_str(), "10.1001/jama.2025.15703");
        assert!(doi.is_borrowed());
        assert_eq!(c.elocation_doi.as_ref().unwrap().valid(), Some(doi));
        assert_eq!(c.pii, Some("2844550"));
        assert_eq!(c.pmc, None);
        assert!(
            matches!(c.published, PartialDate::Day(d) if d.month() == Month::January && d.day().get() == 29)
        );
        assert_eq!(c.electronic.unwrap().to_string(), "2026-01-29");
        assert_eq!(c.revised.unwrap().to_string(), "2026-01-29");
        assert_eq!(c.completed, None);
        assert_eq!(c.languages.len(), 1);
        assert_eq!(c.languages[0].alpha_3(), "eng");
        assert_eq!(c.publication_types.len(), 1);
        assert_eq!(c.publication_types[0].ui.as_str(), "D016428");
        assert_eq!(c.publication_types[0].name, "Journal Article");
        assert!(c.relations.is_empty());
        assert!(out.references.is_empty());
    }

    #[test]
    fn medline_date_becomes_a_span_or_quarter() {
        let input = fixture("medline_date");
        let article = only_article(&input);
        let mut out = Collect::default();
        normalize(&article, &mut out).expect("normalizes");
        assert!(matches!(
            out.citations[0].published,
            PartialDate::Span(_) | PartialDate::YearQuarter(..)
        ));
    }

    #[test]
    fn references_and_relations_come_through() {
        let input = fixture("refs_pmcid_corrections");
        let article = only_article(&input);
        let mut out = Collect::default();
        normalize(&article, &mut out).expect("normalizes");
        let c = &out.citations[0];
        assert!(!out.references.is_empty());
        assert!(out.references.iter().all(|r| r.from == c.pmid));
        assert!(out.references.iter().any(|r| r.pmc.is_some()));
        assert!(out.references.iter().any(|r| r.pmid.is_some()));
        assert!(
            out.references
                .iter()
                .any(|r| matches!(r.doi, Some(Claim::Valid(_))))
        );
        assert!(!c.relations.is_empty());
        assert!(RefType::TOKENS.contains(&c.relations[0].kind.token()));
        assert!(c.relations[0].pmid.is_some());
    }

    #[test]
    fn a_malformed_doi_is_a_claim_not_a_crash() {
        let input = fixture("shortest").replace(
            "<ArticleId IdType=\"doi\">10.1001/jama.2025.15703</ArticleId>",
            "<ArticleId IdType=\"doi\">DOI</ArticleId>",
        );
        let article = only_article(&input);
        let mut out = Collect::default();
        normalize(&article, &mut out).expect("normalizes");
        let c = &out.citations[0];
        assert!(matches!(c.doi, Some(Claim::Malformed { raw: "DOI", .. })));
        assert!(c.elocation_doi.as_ref().unwrap().valid().is_some());
    }

    #[test]
    fn two_valid_but_different_dois_fail_fast() {
        let input = fixture("shortest").replace(
            "<ArticleId IdType=\"doi\">10.1001/jama.2025.15703</ArticleId>",
            "<ArticleId IdType=\"doi\">10.1001/jama.2025.99999</ArticleId>",
        );
        let article = only_article(&input);
        let err = normalize(&article, &mut Collect::default()).unwrap_err();
        assert_eq!(err.pmid, Some(Pmid::new(41_609_757).unwrap()));
        assert_eq!(err.field, "ELocationID[doi]");
        assert!(
            err.to_string()
                .contains("differs from the valid ArticleId doi"),
            "{err}"
        );
    }

    #[test]
    fn an_unknown_date_form_fails_with_the_raw_text() {
        let input = fixture("shortest").replace(
            "<Month>Jan</Month>\n              <Day>29</Day>",
            "<Month>Janvier</Month>\n              <Day>29</Day>",
        );
        assert!(input.contains("Janvier"), "fixture edit applied");
        let article = only_article(&input);
        let err = normalize(&article, &mut Collect::default()).unwrap_err();
        assert_eq!(err.raw, "Janvier");
        assert_eq!(err.field, "JournalIssue/PubDate");
    }
}
