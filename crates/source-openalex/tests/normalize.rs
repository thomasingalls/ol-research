//! The normalizer over the real fixtures: every one normalizes, the strict
//! types hold what the census said they would, and a broken invariant stops
//! with the work's id and the field (plan §6).

use ingest::Sink;
use model::{Claim, DoiError, IssnError, OpenAlexId, OrcidError, Ror};
use smallvec::SmallVec;
use source_openalex::OpenAlex;
use source_openalex::normalize::{Error, ErrorKind, LocationId, Work, normalize};
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
struct Collect<'n> {
    works: Vec<Work<'n>>,
    edges: Vec<(OpenAlexId, OpenAlexId)>,
}

impl<'n> Sink<'n, OpenAlex> for Collect<'n> {
    fn work(&mut self, work: &Work<'n>) {
        self.works.push(work.clone());
    }

    fn reference(&mut self, from: &OpenAlexId, to: &OpenAlexId) {
        self.edges.push((*from, *to));
    }

    fn deletion(&mut self, _: &OpenAlexId) {}
}

fn run<'n>(native: &'n source_openalex::Work<'_>) -> Result<Collect<'n>, Error> {
    let mut sink = Collect::default();
    normalize(native, &mut sink)?;
    Ok(sink)
}

fn one<'n>(native: &'n source_openalex::Work<'_>) -> (Work<'n>, Vec<(OpenAlexId, OpenAlexId)>) {
    let sink = run(native).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(sink.works.len(), 1);
    (sink.works.into_iter().next().unwrap(), sink.edges)
}

const ALL: [&str; 23] = [
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
    "null_institution_ror_type",
    "apc_no_usd",
    "null_title_paratext",
    "null_publication_date",
    "api_content_urls",
    "api_corresponding_institution_no_author",
    "api_content_pdf_only",
    "api_location_without_id",
    "api_best_is_published_false",
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
        assert_eq!(
            w.published.map(|d| d.to_string()),
            native.publication_date.as_ref().map(ToString::to_string),
            "{name}"
        );
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
            native
                .best_oa_location
                .as_ref()
                .is_some_and(|b| b.id.is_some()),
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
    assert!(w.locations.iter().any(|l| {
        matches!(l.id, Some(LocationId::Pmid(_))) && l.provenance == Some(Provenance::Pubmed)
    }));
    assert!(w.doi.is_none() && native.doi.is_none());
}

#[test]
fn an_api_work_types_its_locations_by_namespace_and_says_no_provenance() {
    let input = fixture("api_content_urls");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert!(w.locations.iter().all(|l| l.provenance.is_none()));
    let ids: Vec<&LocationId<'_>> = w.locations.iter().filter_map(|l| l.id.as_ref()).collect();
    assert!(matches!(ids[0], LocationId::Doi(d) if d.as_str() == "10.1093/acamed/wvaf064"));
    assert!(matches!(ids[1], LocationId::Pmid(p) if p.get() == 41_575_249));
    assert!(w.best_oa_location().is_some());
    let pmh = input.replacen("\"id\":\"pmid:41575249\"", "\"id\":\"pmh:oai:x:1\"", 1);
    let native = parse(&pmh);
    let (w, _) = one(&native);
    assert!(
        matches!(w.locations[1].id, Some(LocationId::Repo("oai:x:1"))),
        "pmh: is stripped to the snapshot's oai: form"
    );
    // In primary_location, locations[0] and best_oa_location alike, so
    // the derivable-field checks still pass and the namespace is what fails.
    let broken = input.replace("\"id\":\"doi:", "\"id\":\"isbn:");
    let Err(err) = run(&parse(&broken)) else {
        panic!("an unknown namespace")
    };
    assert_eq!(err.field, "locations.id");
    assert!(matches!(err.kind, ErrorKind::Unmodelled(_)), "{err}");
    let inconsistent = input.replacen(".grobid-xml\"", ".grobid-xml2\"", 1);
    let Err(err) = run(&parse(&inconsistent)) else {
        panic!("a content url off the pattern")
    };
    assert_eq!(err.field, "content_urls");
}

#[test]
fn a_corresponding_institution_list_that_disagrees_is_kept_as_written() {
    let input = fixture("api_corresponding_institution_no_author");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert!(w.authorships.iter().all(|a| !a.is_corresponding));
    let kept = w.corresponding_institutions.as_ref().expect("kept");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].to_string(), "I5950314");
    let input = fixture("api_content_urls");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert!(
        w.corresponding_institutions.is_none(),
        "the consistent case"
    );
}

#[test]
fn the_best_copy_may_spell_unpublished_as_false_where_the_location_says_null() {
    let input = fixture("api_best_is_published_false");
    let native = parse(&input);
    let (w, _) = one(&native);
    let best = w.best_oa_location().expect("open");
    assert!(best.version.is_none());
    assert!(matches!(best.id, Some(LocationId::Doi(_))));
}

#[test]
fn a_location_may_have_no_id_at_all() {
    let input = fixture("api_location_without_id");
    let native = parse(&input);
    let (w, _) = one(&native);
    assert_eq!(w.locations.iter().filter(|l| l.id.is_none()).count(), 1);
    assert!(w.locations.iter().any(|l| l.id.is_some()));
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
            .any(|l| matches!(l.id, Some(LocationId::Repo(_))))
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
            .and_then(|i| native.locations[i].id.as_deref()),
        native
            .best_oa_location
            .as_ref()
            .and_then(|b| b.id.as_deref())
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
            .any(|l| matches!(l.id, Some(LocationId::Mag(_))))
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
                match (i.ror, &ni.ror) {
                    (Some(ror), Some(raw)) => {
                        assert_eq!(format!("{}{ror}", Ror::URL_PREFIX), *raw, "{name}");
                        rors += 1;
                    }
                    (None, None) => assert!(i.kind.is_none(), "{name}: no ror, no type"),
                    _ => panic!("{name}: ror presence differs between native and normalized"),
                }
                assert!(i.lineage.contains(&i.id));
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
                    ns.issn.as_ref().map_or(0, SmallVec::len),
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

    // A checksum failure in an ORCID is a claim, not a stop, since the
    // API sent `https://orcid.org/` with nothing after it (2026-09-15).
    let name = ALL
        .iter()
        .find(|n| fixture(n).contains("\"orcid\":\"https://orcid.org/"))
        .expect("a fixture with an ORCID");
    let input = fixture(name);
    let at = input.find("\"orcid\":\"https://orcid.org/").unwrap()
        + "\"orcid\":\"https://orcid.org/".len()
        + 18;
    let flipped = if &input[at..=at] == "0" { "1" } else { "0" };
    let mutated = format!("{}{flipped}{}", &input[..at], &input[at + 1..]);
    let native = parse(&mutated);
    let (w, _) = one(&native);
    assert!(
        w.authorships.iter().any(|a| matches!(
            a.author.orcid,
            Some(Claim::Malformed {
                error: OrcidError::Checksum,
                ..
            })
        )),
        "{name}"
    );
}

#[test]
fn the_binary_layout_roundtrips_every_fixture() {
    use ingest::{Binary, Kind, Reader};
    let prefix = std::env::temp_dir().join(format!("ol-rht-openalex-{}", std::process::id()));
    let mut binary = Binary::new("openalex", model::VERSION, {
        let prefix = prefix.clone();
        move |_: usize, _: &std::path::Path| prefix.clone()
    });
    binary
        .open_file(std::path::Path::new("fixtures"), 0)
        .unwrap();
    let inputs: Vec<String> = ALL.iter().map(|n| fixture(n)).collect();
    let natives: Vec<source_openalex::Work<'_>> = inputs.iter().map(|i| parse(i)).collect();
    let mut collect = Collect::default();
    for native in &natives {
        normalize(native, &mut (&mut collect, &mut binary)).unwrap();
    }
    binary.close_file().unwrap();
    let written = binary.finish().unwrap();
    assert_eq!(written.len(), 1);
    let written = &written[0];
    assert_eq!(written.records[0], ALL.len() as u64);
    assert_eq!(written.records[1], collect.edges.len() as u64);
    let works = Reader::open(&Kind::Works.path(&prefix)).unwrap();
    assert_eq!(works.header().model, model::VERSION);
    let back: Vec<Work<'_>> = works.records().map(Result::unwrap).collect();
    // Interned strings come back as symbols; through the set's table they
    // serialize to the same text.
    assert!(works.table().is_some_and(|t| !t.is_empty()));
    assert_eq!(
        works.with_table(|| serde_json::to_string(&back).unwrap()),
        serde_json::to_string(&collect.works).unwrap()
    );
    let refs = Reader::open(&Kind::References.path(&prefix)).unwrap();
    let back: Vec<(OpenAlexId, OpenAlexId)> = refs.records().map(Result::unwrap).collect();
    assert_eq!(back, collect.edges);
    for kind in Kind::ALL {
        std::fs::remove_file(kind.path(&prefix)).unwrap();
    }
}
