//! The normalizer over the real fixtures: every one normalizes, the strict
//! types hold what the census said they would, and a broken invariant stops
//! with the work's id and the field (plan §6).

use model::{Claim, DoiError, IssnError, OpenAlexId, OrcidError, Ror};
use source_openalex::normalize::{Emit, Error, ErrorKind, LocationId, Work, normalize};
use source_openalex::vocab::{AuthorPosition, Index, OaStatus, Provenance, WorkType};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/fixtures/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn parse(input: &str) -> source_openalex::Work<'_> {
    serde_json::from_str(input).expect("parses")
}

#[derive(Default)]
struct Sink<'n> {
    works: Vec<Work<'n>>,
    edges: Vec<(OpenAlexId, OpenAlexId)>,
}

impl<'n> Emit<'n> for Sink<'n> {
    fn work(&mut self, work: &Work<'n>) {
        self.works.push(work.clone());
    }

    fn reference(&mut self, from: OpenAlexId, to: OpenAlexId) {
        self.edges.push((from, to));
    }
}

fn run<'n>(native: &'n source_openalex::Work<'_>) -> Result<Sink<'n>, Error> {
    let mut sink = Sink::default();
    normalize(native, &mut sink)?;
    Ok(sink)
}

fn one<'n>(native: &'n source_openalex::Work<'_>) -> (Work<'n>, Vec<(OpenAlexId, OpenAlexId)>) {
    let sink = run(native).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(sink.works.len(), 1);
    (sink.works.into_iter().next().unwrap(), sink.edges)
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
fn every_fixture_normalizes_with_its_edges() {
    for name in ALL {
        let input = fixture(name);
        let native = parse(&input);
        let sink = run(&native).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(sink.works.len(), 1, "{name}");
        let w = &sink.works[0];
        assert_eq!(
            format!("{}{}", OpenAlexId::URL_PREFIX, w.id),
            native.id,
            "{name}"
        );
        assert_eq!(
            sink.edges.len(),
            w.referenced_works_count as usize,
            "{name}"
        );
        assert!(sink.edges.iter().all(|(from, _)| *from == w.id), "{name}");
        assert_eq!(w.locations.len(), native.locations.len(), "{name}");
        assert_eq!(w.authorships.len(), native.authorships.len(), "{name}");
        assert_eq!(w.published.to_string(), native.publication_date, "{name}");
        assert_eq!(
            w.created.date().to_string(),
            &native.created_date[..10],
            "{name}"
        );
        assert!(w.updated >= w.created, "{name}");
        assert_eq!(w.is_paratext(), native.is_paratext, "{name}");
        assert_eq!(w.open_access.is_oa(), native.open_access.is_oa, "{name}");
        assert_eq!(
            w.best_oa_location().is_some(),
            native.best_oa_location.id.is_some(),
            "{name}"
        );
        match (&w.doi, native.doi.as_deref()) {
            (Some(Claim::Valid(d)), Some(url)) => {
                assert_eq!(format!("{}{d}", model::Doi::URL_PREFIX), url, "{name}");
            }
            (Some(Claim::Malformed { .. }), Some(_)) | (None, None) => {}
            other => panic!("{name}: {other:?}"),
        }
        assert_eq!(w.clone(), *w, "{name}");
    }
}

#[test]
fn identifiers_are_strict_types() {
    let input = fixture("pubmed_with_mesh");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert!(w.pmid.is_some());
    assert!(w.indexed_in.contains(Index::Pubmed));
    assert!(!w.mesh.is_empty());
    assert!(
        w.mesh
            .iter()
            .all(|m| m.descriptor.as_str().starts_with('D'))
    );
    assert!(
        w.mesh
            .iter()
            .filter_map(|m| m.qualifier)
            .all(|q| q.ui.as_str().starts_with('Q'))
    );
    assert!(
        w.locations
            .iter()
            .any(|l| { matches!(l.id, LocationId::Pmid(_)) && l.provenance == Provenance::Pubmed })
    );
    assert!(w.doi.is_none() && native.doi.is_none());
}

#[test]
fn locations_carry_their_provenance_typed_id() {
    let input = fixture("dissertation_repo_pdf");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert_eq!(w.kind, WorkType::Dissertation);
    assert!(
        w.locations
            .iter()
            .any(|l| matches!(l.id, LocationId::Repo(_)))
    );
    let best = w.best_oa_location().expect("open");
    assert!(best.is_oa);
    assert!(std::ptr::eq(
        best,
        w.locations.iter().find(|l| l.is_oa).unwrap()
    ));
    assert_eq!(
        w.locations
            .iter()
            .position(|l| l.is_oa)
            .map(|i| &native.locations[i].id[..]),
        native.best_oa_location.id.as_deref()
    );
    let input = fixture("closed_no_oa_location");
    let native = parse(&input);
    let (w, edges) = one(&native);
    assert_eq!(w.open_access.status, OaStatus::Closed);
    assert!(!w.open_access.is_oa());
    assert!(w.best_oa_location().is_none());
    assert!(!edges.is_empty());
    assert!(
        w.locations
            .iter()
            .any(|l| matches!(l.id, LocationId::Mag(_)))
    );
}

#[test]
fn the_data_being_wrong_is_a_claim() {
    let input = fixture("empty_doi");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert!(matches!(
        w.doi,
        Some(Claim::Malformed {
            raw: "",
            error: DoiError::Empty
        })
    ));

    let input = fixture("bad_issn");
    let native = parse(&input);
    let (w, _) = one(&native);
    let malformed: Vec<_> = w
        .locations
        .iter()
        .filter_map(|l| l.source.as_ref())
        .flat_map(|s| s.issns.iter())
        .filter(|c| c.is_malformed())
        .collect();
    assert!(
        malformed.iter().any(|c| matches!(
            c,
            Claim::Malformed {
                error: IssnError::Checksum,
                ..
            }
        )),
        "{malformed:?}"
    );

    let input = fixture("language_sh");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert!(matches!(
        w.language,
        Some(Claim::Malformed { raw: "sh", .. })
    ));
}

#[test]
fn a_language_without_a_two_letter_code_is_written_in_three() {
    let input = fixture("language_ceb");
    let native = parse(&input);
    let (w, _) = one(&native);
    match w.language {
        Some(Claim::Valid(l)) => {
            assert_eq!(l.alpha_3(), "ceb");
            assert_eq!(l.alpha_2(), None);
        }
        other => panic!("{other:?}"),
    }
    let input = fixture("smallest");
    let native = parse(&input);
    let (w, _) = one(&native);
    if let Some(Claim::Valid(l)) = w.language {
        assert_eq!(l.alpha_2(), native.language.as_deref());
    }
}

#[test]
fn what_the_census_said_was_not_derivable() {
    let input = fixture("mag_differs");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert_ne!(u64::from(w.mag.unwrap().get()), w.id.get());

    let input = fixture("lone_last_author");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert_eq!(w.authorships.len(), 1);
    assert_eq!(w.authorships[0].position, AuthorPosition::Last);

    let input = fixture("self_reference");
    let native = parse(&input);
    let (w, edges) = one(&native);
    assert!(edges.iter().any(|(from, to)| from == to));
    assert_eq!(edges.len(), w.referenced_works_count as usize);

    let input = fixture("null_author_id_no_abstract");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert!(w.authorships.iter().any(|a| a.author.id.is_none()));
    assert!(w.abstract_inverted_index.is_none());
}

#[test]
fn funding_and_charges() {
    let input = fixture("awards_and_funders");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert!(!w.awards.is_empty());
    for a in &w.awards {
        assert!(w.funders.iter().any(|f| f.id == a.funder));
    }
    let input = fixture("apc_paid");
    let native = parse(&input);
    let (w, _) = one(&native);
    let paid = w.apc_paid.expect("paid");
    assert_eq!(paid.currency.as_str(), "USD");
    assert_eq!(
        f64::from(paid.value).to_bits(),
        native.apc_paid.as_ref().unwrap().value.to_bits()
    );
}

#[test]
fn rors_and_host_organizations_roundtrip() {
    let mut rors = 0;
    let mut hosts = 0;
    for name in ALL {
        let input = fixture(name);
        let native = parse(&input);
        let (w, _) = one(&native);
        for (a, na) in w.authorships.iter().zip(&native.authorships) {
            for (i, ni) in a.institutions.iter().zip(&na.institutions) {
                assert_eq!(format!("{}{}", Ror::URL_PREFIX, i.ror), ni.ror, "{name}");
                assert!(i.lineage.contains(&i.id));
                rors += 1;
            }
            assert_eq!(a.countries.len(), na.countries.len());
        }
        for (l, nl) in w.locations.iter().zip(&native.locations) {
            if let (Some(s), Some(ns)) = (&l.source, &nl.source) {
                assert_eq!(
                    s.host_organization()
                        .map(|h| format!("{}{h}", OpenAlexId::URL_PREFIX)),
                    ns.host_organization.as_deref().map(str::to_owned),
                    "{name}"
                );
                hosts += u64::from(s.host_organization().is_some());
                assert_eq!(
                    s.issns.len(),
                    ns.issn.as_ref().map_or(0, Vec::len),
                    "{name}"
                );
            }
        }
    }
    assert!(rors > 0 && hosts > 0, "{rors} rors, {hosts} hosts");
}

/// Applies `mutate` to a fixture and expects normalization to stop at
/// `field`.
fn expect_failure(name: &str, mutate: impl Fn(&str) -> String, field: &str) -> Error {
    let input = fixture(name);
    let mutated = mutate(&input);
    assert_ne!(mutated, input, "{name}: the mutation did not apply");
    let native = parse(&mutated);
    let err = run(&native)
        .err()
        .unwrap_or_else(|| panic!("{name}: {field} did not fail"));
    assert_eq!(err.field, field, "{err}");
    assert!(err.id.is_some(), "{err}");
    assert!(err.to_string().starts_with('W'), "{err}");
    err
}

#[test]
fn broken_invariants_stop_with_id_and_field() {
    let err = expect_failure(
        "smallest",
        |s| {
            let re = s.find("\"publication_year\":").unwrap() + "\"publication_year\":".len();
            let end = re + s[re..].find(',').unwrap();
            format!("{}1234{}", &s[..re], &s[end..])
        },
        "publication_year",
    );
    assert!(matches!(err.kind, ErrorKind::Inconsistent(_)), "{err}");

    let err = expect_failure(
        "smallest",
        |s| s.replacen("\"display_name\":\"", "\"display_name\":\"x", 1),
        "display_name",
    );
    assert!(matches!(err.kind, ErrorKind::Inconsistent(_)), "{err}");

    let err = expect_failure(
        "smallest",
        |s| {
            let re = s.find("\"authors_count\":").unwrap() + "\"authors_count\":".len();
            let end = re + s[re..].find(',').unwrap();
            format!("{}99{}", &s[..re], &s[end..])
        },
        "authors_count",
    );
    assert!(matches!(err.kind, ErrorKind::Inconsistent(_)), "{err}");

    let err = expect_failure(
        "smallest",
        |s| {
            let re = s.find("\"updated_date\":\"").unwrap() + "\"updated_date\":\"".len();
            format!("{}1900-01-01T00:00:00.000Z{}", &s[..re], &s[re + 24..])
        },
        "updated_date",
    );
    assert!(matches!(err.kind, ErrorKind::Inconsistent(_)), "{err}");

    let err = expect_failure(
        "closed_no_oa_location",
        |s| {
            s.replacen(
                "\"referenced_works\":[\"https://openalex.org/W",
                "\"referenced_works\":[\"https://openalex.org/w",
                1,
            )
        },
        "referenced_works",
    );
    assert!(
        matches!(err.kind, ErrorKind::Id(model::ParseError::BadPrefix)),
        "{err}"
    );

    // A checksum failure in an ORCID stops the run: the sample earned the
    // strict type.
    let name = ALL
        .iter()
        .find(|n| fixture(n).contains("\"orcid\":\"https://orcid.org/"))
        .expect("a fixture with an ORCID");
    let err = expect_failure(
        name,
        |s| {
            let at = s.find("\"orcid\":\"https://orcid.org/").unwrap()
                + "\"orcid\":\"https://orcid.org/".len()
                + 18;
            let flipped = if &s[at..=at] == "0" { "1" } else { "0" };
            format!("{}{flipped}{}", &s[..at], &s[at + 1..])
        },
        "author.orcid",
    );
    assert!(
        matches!(err.kind, ErrorKind::Orcid(OrcidError::Checksum)),
        "{err}"
    );
}
