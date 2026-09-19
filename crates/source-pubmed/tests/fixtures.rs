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
        "nihms_mid",
        "ref_pmc_id",
        "number_of_references",
        "season_month_range",
        "season_slash",
        "season_autumn",
        "two_piis",
        "two_dois",
        "other_id",
        "medline_id",
        "gene_symbols",
        "mid_cams",
        "other_abstract_lang_sp",
        "personal_name_subject",
        "sici",
        "general_note",
        "zero_references",
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
    use ingest::{Sink, Source};
    use model::{Claim, Month, PartialDate, Pmid};
    use source_pubmed::Pubmed;
    use source_pubmed::attr::{
        AffiliationIdSource, IssnType, PubStatus, PublicationStatus, RefType,
    };
    use source_pubmed::normalize::{AuthorName, Citation, Reference, normalize};

    #[derive(Default)]
    struct Collect<'n> {
        citations: Vec<Citation<'n>>,
        froms: Vec<Pmid>,
        references: Vec<Reference<'n>>,
        deletions: Vec<Pmid>,
    }

    impl<'n> Sink<'n, Pubmed> for Collect<'n> {
        fn work(&mut self, citation: &Citation<'n>) {
            self.citations.push(citation.clone());
        }
        fn reference(&mut self, from: &Pmid, reference: &Reference<'n>) {
            self.froms.push(*from);
            self.references.push(reference.clone());
        }
        fn deletion(&mut self, id: &Pmid) {
            self.deletions.push(*id);
        }
    }

    #[test]
    fn deletions_are_a_stream_of_pmids() {
        let input = fixture("shortest").replacen(
            "</PubmedArticleSet>",
            "<DeleteCitation><PMID Version=\"1\">41609757</PMID><PMID Version=\"1\">12</PMID></DeleteCitation></PubmedArticleSet>",
            1,
        );
        let records = super::all_records(&input);
        assert_eq!(records.len(), 2);
        let mut out = Collect::default();
        for r in &records {
            Pubmed::normalize(r, &mut out).expect("normalizes");
        }
        assert_eq!(out.citations.len(), 1);
        assert_eq!(
            out.deletions,
            [Pmid::new(41_609_757).unwrap(), Pmid::new(12).unwrap()]
        );
        let bad = input.replacen(
            "<PMID Version=\"1\">12</PMID>",
            "<PMID Version=\"1\">012</PMID>",
            1,
        );
        let records = super::all_records(&bad);
        let err = Pubmed::normalize(&records[1], &mut Collect::default()).unwrap_err();
        assert_eq!(err.field, "DeleteCitation/PMID");
    }

    #[test]
    fn an_author_manuscript_carries_its_nihms_id() {
        let out = run("nihms_mid");
        assert_eq!(out.citations.len(), 1);
        assert_eq!(out.citations[0].ids.mid, Some("NIHMS2140043"));
    }

    fn run(name: &str) -> Collect<'static> {
        let input = Box::leak(fixture(name).into_boxed_str());
        let article = Box::leak(Box::new(only_article(input)));
        let mut out = Collect::default();
        normalize(article, &mut out).expect("normalizes");
        out
    }

    #[test]
    fn shortest_record_normalizes_every_field() {
        let out = run("shortest");
        assert_eq!(out.citations.len(), 1);
        let c = &out.citations[0];
        assert_eq!(c.pmid, Pmid::new(41_609_757).unwrap());
        assert_eq!(c.version, 1);
        let doi = c.ids.doi.as_ref().unwrap().valid().unwrap();
        assert_eq!(doi.as_str(), "10.1001/jama.2025.15703");
        assert!(doi.is_borrowed());
        assert_eq!(c.ids.elocation_doi[0].valid(), Some(doi));
        assert_eq!(c.ids.pii.as_slice(), ["2844550"]);
        assert!(c.ids.pmc.is_none());
        assert!(
            matches!(c.dates.published, PartialDate::Day(d) if d.month() == Month::January && d.day().get() == 29)
        );
        assert_eq!(c.dates.electronic.unwrap().to_string(), "2026-01-29");
        assert_eq!(c.dates.revised.unwrap().to_string(), "2026-01-29");
        assert_eq!(c.title, "Glucose as a Food.");
        assert_eq!(c.journal.title, Some("JAMA".into()));
        assert_eq!(c.journal.medline_ta, "JAMA");
        assert_eq!(c.journal.nlm_unique_id, Some("7501160".into()));
        let issn = c.journal.issn.as_ref().unwrap();
        assert_eq!(issn.kind, IssnType::Electronic);
        assert_eq!(issn.issn.valid().unwrap().to_string(), "1538-3598");
        assert_eq!(
            c.journal
                .issn_linking
                .as_ref()
                .unwrap()
                .valid()
                .unwrap()
                .to_string(),
            "0098-7484"
        );
        assert_eq!(c.journal.country, Some("United States".into()));
        assert_eq!(c.languages[0].alpha_3(), "eng");
        assert_eq!(c.publication_types[0].ui.as_str(), "D016428");
        assert_eq!(c.publication_types[0].name, "Journal Article");
        assert!(c.authors.is_empty() && c.r#abstract.is_none() && c.mesh.is_empty());
        assert_eq!(&*c.citation_subsets, ["IM"]);
        assert_eq!(c.publication_status, Some(PublicationStatus::AheadOfPrint));
        assert_eq!(c.history.len(), 1);
        assert_eq!(c.history[0].status, PubStatus::Entrez);
        assert_eq!(c.history[0].time.unwrap().to_string(), "12:36");
        assert!(out.references.is_empty());
    }

    #[test]
    fn rich_record_has_abstract_sections_mesh_and_authors() {
        let out = run("mathml_refs_mesh");
        let c = &out.citations[0];
        let a = c.r#abstract.as_ref().unwrap();
        assert!(a.sections.iter().any(|s| s.text.contains("<mml:math")));
        assert!(!c.mesh.is_empty());
        assert!(
            c.mesh
                .iter()
                .all(|h| h.descriptor.ui.as_str().starts_with('D'))
        );
        assert!(!c.authors.is_empty());
        assert!(
            c.authors
                .iter()
                .any(|a| matches!(a.name, AuthorName::Person(_)))
        );
        assert!(!out.references.is_empty());
    }

    #[test]
    fn medline_date_becomes_a_span_or_quarter() {
        let out = run("medline_date");
        assert!(matches!(
            out.citations[0].dates.published,
            PartialDate::Span(_) | PartialDate::YearQuarter(..)
        ));
    }

    #[test]
    fn collective_names_come_through() {
        let out = run("collective_name");
        assert!(
            out.citations[0]
                .authors
                .iter()
                .any(|a| matches!(a.name, AuthorName::Collective { .. }))
        );
    }

    #[test]
    fn references_and_relations_come_through() {
        let out = run("refs_pmcid_corrections");
        let c = &out.citations[0];
        assert!(out.froms.iter().all(|from| *from == c.pmid));
        assert_eq!(out.froms.len(), out.references.len());
        assert!(out.references.iter().any(|r| r.pmc.is_some()));
        assert!(
            out.references
                .iter()
                .any(|r| matches!(r.doi, Some(Claim::Valid(_))))
        );
        assert!(!c.relations.is_empty());
        assert!(RefType::TOKENS.contains(&c.relations[0].kind.token()));
    }

    #[test]
    fn orcids_in_three_forms_and_truncated_ones() {
        let base = fixture("shortest");
        let author = |orcid: &str| {
            base.replacen(
            "<ArticleTitle>Glucose as a Food.</ArticleTitle>",
            &format!("<ArticleTitle>Glucose as a Food.</ArticleTitle><AuthorList CompleteYN=\"Y\"><Author ValidYN=\"Y\"><LastName>Doe</LastName><Identifier Source=\"ORCID\">{orcid}</Identifier><AffiliationInfo><Affiliation>Somewhere</Affiliation><Identifier Source=\"ROR\">https://ror.org/05byvp690</Identifier></AffiliationInfo></Author></AuthorList>"),
            1,
        )
        };
        for form in [
            "https://orcid.org/0000-0002-3118-2156",
            "http://orcid.org/0000-0002-3118-2156",
            "0000-0002-3118-2156",
        ] {
            let input = author(form);
            let article = only_article(&input);
            let mut out = Collect::default();
            normalize(&article, &mut out).expect(form);
            let a = &out.citations[0].authors[0];
            assert_eq!(
                a.orcid.as_ref().unwrap().valid().unwrap().to_string(),
                "0000-0002-3118-2156",
                "{form}"
            );
            assert_eq!(a.affiliations[0].ids[0].source, AffiliationIdSource::Ror);
        }
        let input = author("0000-0002-7368-498");
        let article = only_article(&input);
        let mut out = Collect::default();
        normalize(&article, &mut out).unwrap();
        assert!(matches!(
            out.citations[0].authors[0].orcid,
            Some(Claim::Malformed {
                raw: "0000-0002-7368-498",
                ..
            })
        ));
    }

    #[test]
    fn unmodelled_content_fails_on_first_sight() {
        // GeneSymbolList was the example until the looked-up records of
        // 2026-09-15 modelled it; SpaceFlightMission is still unseen.
        let input = fixture("shortest").replace(
            "<CitationSubset>IM</CitationSubset>",
            "<CitationSubset>IM</CitationSubset><SpaceFlightMission>Apollo 11</SpaceFlightMission>",
        );
        let article = only_article(&input);
        let err = normalize(&article, &mut Collect::default()).unwrap_err();
        assert_eq!(err.field, "SpaceFlightMission", "{err}");
        let input = fixture("shortest").replace(
            "<Minute>36</Minute>",
            "<Minute>36</Minute><Second>1</Second>",
        );
        let article = only_article(&input);
        let err = normalize(&article, &mut Collect::default()).unwrap_err();
        assert!(err.to_string().contains("Second"), "{err}");
    }

    #[test]
    fn two_valid_but_different_dois_are_both_kept() {
        // Both slots are names of the work (merge plan §3.3): at Crossref
        // either may be the registered DOI, and 19257808 has both, an
        // early-online one and the issue's (2026-09-18).
        let input = fixture("shortest").replace(
            "<ArticleId IdType=\"doi\">10.1001/jama.2025.15703</ArticleId>",
            "<ArticleId IdType=\"doi\">10.1001/jama.2025.99999</ArticleId>",
        );
        let article = only_article(&input);
        let mut out = Collect::default();
        normalize(&article, &mut out).unwrap();
        let ids = &out.citations[0].ids;
        assert_eq!(
            ids.doi.as_ref().unwrap().valid().unwrap().as_str(),
            "10.1001/jama.2025.99999"
        );
        assert_eq!(
            ids.elocation_doi[0].valid().unwrap().as_str(),
            "10.1001/jama.2025.15703"
        );
    }

    #[test]
    fn a_pmc_slot_that_is_not_a_pmc_id_is_kept_as_written() {
        // PMID 34850701 in pubmed26n1339: the publisher's own item id,
        // which its other records give as their `pii` (2026-09-18).
        let input = fixture("shortest").replace(
            "<ArticleId IdType=\"doi\">10.1001/jama.2025.15703</ArticleId>",
            "<ArticleId IdType=\"doi\">10.1001/jama.2025.15703</ArticleId><ArticleId IdType=\"pmc\">wst_2021_260</ArticleId>",
        );
        let article = only_article(&input);
        let mut out = Collect::default();
        normalize(&article, &mut out).unwrap();
        let pmc = out.citations[0].ids.pmc.as_ref().unwrap();
        assert!(pmc.valid().is_none());
        assert!(matches!(pmc, Claim::Malformed { raw, .. } if *raw == "wst_2021_260"));
    }

    #[test]
    fn a_record_may_give_more_than_one_publisher_item_id() {
        // PMID 32839689 in pubmed26n1336: Elsevier's item id and the
        // article number (2026-09-18).
        let input = fixture("shortest").replace(
            "<ArticleId IdType=\"pii\">2844550</ArticleId>",
            "<ArticleId IdType=\"pii\">2844550</ArticleId><ArticleId IdType=\"pii\">S2213-5766(20)30238-4</ArticleId>",
        );
        let article = only_article(&input);
        let mut out = Collect::default();
        normalize(&article, &mut out).unwrap();
        assert_eq!(
            out.citations[0].ids.pii.as_slice(),
            ["2844550", "S2213-5766(20)30238-4"]
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

    #[test]
    fn the_binary_layout_roundtrips_the_fixtures() {
        use super::{fixture, only_article};
        use ingest::{Binary, Kind, Reader};
        let prefix = std::env::temp_dir().join(format!("ol-rht-pubmed-{}", std::process::id()));
        let mut binary = Binary::new("pubmed", model::VERSION, {
            let prefix = prefix.clone();
            move |_: usize, _: &std::path::Path| prefix.clone()
        });
        binary
            .open_file(std::path::Path::new("fixtures"), 0)
            .unwrap();
        let inputs: Vec<String> = ["shortest", "mathml_refs_mesh", "refs_pmcid_corrections"]
            .iter()
            .map(|n| fixture(n))
            .collect();
        let articles: Vec<_> = inputs.iter().map(|i| only_article(i)).collect();
        let mut collect = Collect::default();
        for article in &articles {
            normalize(article, &mut (&mut collect, &mut binary)).expect("normalizes");
        }
        binary.close_file().unwrap();
        let written = binary.finish().unwrap();
        assert_eq!(written.len(), 1);
        let written = &written[0];
        assert_eq!(written.records[0], 3);
        assert_eq!(written.records[1], collect.references.len() as u64);
        let works = Reader::open(&Kind::Works.path(&prefix)).unwrap();
        let back: Vec<Citation<'_>> = works.records().map(Result::unwrap).collect();
        assert_eq!(back.len(), 3);
        for (a, b) in back.iter().zip(&collect.citations) {
            assert_eq!(a.pmid, b.pmid);
            assert_eq!(a.title, b.title);
            assert_eq!(a.authors.len(), b.authors.len());
            assert_eq!(a.mesh.len(), b.mesh.len());
            assert_eq!(a.dates.published, b.dates.published);
        }
        let refs = Reader::open(&Kind::References.path(&prefix)).unwrap();
        {
            let back: Vec<(Pmid, Reference<'_>)> = refs.records().map(Result::unwrap).collect();
            assert_eq!(
                back.iter().map(|(from, _)| *from).collect::<Vec<_>>(),
                collect.froms
            );
            // Compared through Debug: `==` would tie the lifetime of the
            // rows read back to the sink's, which outlives the reader.
            assert!(
                back.iter()
                    .zip(&collect.references)
                    .all(|((_, a), b)| format!("{a:?}") == format!("{b:?}"))
            );
        }
        for kind in Kind::NORMALIZED {
            std::fs::remove_file(kind.path(&prefix)).unwrap();
        }
    }
}

/// Every record of a whole document, through the whole-document reader.
fn all_records(input: &str) -> Vec<Record<'_>> {
    let mut set = ArticleSet::new(input);
    let mut records = Vec::new();
    while let Some(record) = set.next_record().expect("parses") {
        records.push(record);
    }
    records
}

mod streaming {
    use std::io::Cursor;
    use std::ops::ControlFlow;

    use ingest::ReadError;
    use source_pubmed::{ArticleSet, ErrorKind, records};

    use super::{all_records, fixture};

    /// Debug renderings of every record, as the streaming reader sees them.
    fn streamed(input: &str) -> Result<Vec<String>, source_pubmed::Error> {
        let mut out = Vec::new();
        match records(Cursor::new(input), |r| {
            out.push(format!("{r:?}"));
            ControlFlow::Continue(())
        }) {
            Ok(()) => Ok(out),
            Err(ReadError::Parse(e)) => Err(e),
            Err(ReadError::Io(e)) => panic!("{e}"),
        }
    }

    #[test]
    fn the_stream_reads_exactly_what_the_whole_document_reader_reads() {
        for name in [
            "shortest",
            "collective_name",
            "mathml_refs_mesh",
            "medline_date",
            "refs_pmcid_corrections",
        ] {
            let input = fixture(name);
            let whole: Vec<String> = all_records(&input)
                .iter()
                .map(|r| format!("{r:?}"))
                .collect();
            assert_eq!(streamed(&input).unwrap(), whole, "{name}");
        }
        let input = fixture("shortest").replacen(
            "</PubmedArticleSet>",
            "<DeleteCitation><PMID Version=\"1\">41609757</PMID><PMID Version=\"1\">12</PMID></DeleteCitation></PubmedArticleSet>",
            1,
        );
        let whole: Vec<String> = all_records(&input)
            .iter()
            .map(|r| format!("{r:?}"))
            .collect();
        assert_eq!(whole.len(), 2);
        assert_eq!(streamed(&input).unwrap(), whole);
    }

    #[test]
    fn the_stream_reports_the_same_error_at_the_same_file_offset() {
        let input = fixture("shortest").replacen(
            "</PubmedArticleSet>",
            "<DeleteCitation><PMID Version=\"1\">41609757</PMID><PMIDX Version=\"1\">12</PMIDX></DeleteCitation></PubmedArticleSet>",
            1,
        );
        let whole = ArticleSet::new(&input);
        let mut whole = whole;
        whole.next_record().expect("the article parses");
        let expected = whole.next_record().unwrap_err();
        let err = streamed(&input).unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::UnexpectedElement(ref n) if n == "PMIDX"),
            "{err}"
        );
        assert_eq!(
            (err.position, err.path, err.pmid),
            (expected.position, expected.path, expected.pmid)
        );
        assert!(err.position > input.find("<DeleteCitation>").unwrap() as u64);

        let input = fixture("shortest").replacen("<ArticleTitle>", "<ArticleTitel>", 1);
        let expected = ArticleSet::new(&input).next_record().unwrap_err();
        let err = streamed(&input).unwrap_err();
        assert_eq!(
            (err.position, err.path, err.pmid),
            (expected.position, expected.path, expected.pmid)
        );
    }
}
