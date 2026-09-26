//! Records from Crossref's March 2026 public data file, each the line as
//! shipped, pulled off the box where the survey of the whole file stopped
//! (homelab `ol-rht-dump-crossref-v1`, 2026-09-26): one per shape the
//! survey found, and the first regression fixture for it (plan §6).

use ingest::Sink;
use merge::OwnedName;
use merge::kind::Genre;
use merge::relation::{Kind, Related, Update};
use model::{Claim, Doi, RorError, Table};
use source_crossref::normalize::{Institution, Reference, Work, normalize};
use source_crossref::view;
use source_crossref::vocab::{
    AssertedBy, FundingType, ReferenceType, RelationIdKind, ReviewRecommendation, UpdateType,
    UpdateWord,
};

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
    excluded: Vec<&'static str>,
    set_aside: Vec<(&'static str, &'static str)>,
}

impl<'n> Sink<'n, source_crossref::Crossref> for Collect<'n> {
    fn work(&mut self, w: &Work<'n>) {
        self.works.push(w.clone());
    }
    fn reference(&mut self, _: &Doi<'n>, r: &Reference<'n>) {
        self.references.push(r.clone());
    }
    fn deletion(&mut self, _: &Doi<'n>) {}
    fn excluded(&mut self, _: &Doi<'n>, reason: &'static str) {
        self.excluded.push(reason);
    }
    fn set_aside(&mut self, _: &Doi<'n>, field: &'static str, value: &'static str) {
        self.set_aside.push((field, value));
    }
}

fn native(name: &str) -> &'static source_crossref::Work<'static> {
    let input = Box::leak(fixture(name).into_boxed_str());
    Box::leak(Box::new(serde_json::from_str(input).expect("parses")))
}

fn collect(name: &str) -> Collect<'static> {
    let mut out = Collect::default();
    normalize(native(name), &mut out).unwrap_or_else(|e| panic!("{name}: {e}"));
    out
}

fn run(name: &str) -> Work<'static> {
    let mut out = collect(name);
    assert_eq!(out.works.len(), 1, "{name}");
    out.works.remove(0)
}

/// The merge's reading of the record: its relations.
fn relations(w: &Work<'_>) -> Vec<Related> {
    let mut out = Vec::new();
    view::relations(w, &Table::default(), &mut |r| out.push(r)).unwrap();
    out
}

/// The merge's view of the record, against an empty table: the fixtures'
/// text is never interned.
fn view_of<'w>(w: &'w Work<'_>) -> merge::View<'w> {
    static TABLE: std::sync::LazyLock<Table> = std::sync::LazyLock::new(Table::default);
    view::view(w, &TABLE).unwrap()
}

/// A relation's target, as the merge holds one.
#[allow(clippy::unnecessary_wraps)] // compared with `Related::target`
fn doi(text: &str) -> Option<OwnedName> {
    Some(OwnedName::Doi(text.into()))
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

/// 10.6019/pxd000010, shard 25589 line 4098: an EMBL-EBI `ProteomeXchange`
/// dataset page with no `type`, as 2,257 of the records the survey
/// stopped at are. Unknown, not guessed: no form, and no genre.
#[test]
fn a_record_without_a_type_has_no_form_and_no_genre() {
    let w = run("work_without_type");
    assert_eq!(w.kind, None);
    let v = view_of(&w);
    assert_eq!((v.form, v.genre), (None, None));
}

/// 10.1016/j.homp.2015.03.001, shard 22065 line 443: Elsevier's `err`,
/// on a notice titled "Corrigendum to …": an erratum, the word kept.
#[test]
fn elsevier_err_is_an_erratum_notice_with_its_word_kept() {
    let w = run("update_to_err");
    let u = &w.updates_to[0];
    assert_eq!((u.kind, u.word), (UpdateType::Erratum, UpdateWord::Err));
    assert_eq!(view::genre(&w), Some(Genre::Erratum));
    assert_eq!(
        relations(&w),
        [Related {
            kind: Kind::UpdateTo(Update::Erratum),
            target: doi("10.1016/j.homp.2014.12.001")
        }]
    );
}

/// 10.1016/j.jcma.2013.06.001, shard 24354 line 969: the article the
/// `err` notice corrects, with the update copied into its `relation`.
#[test]
fn an_article_elsevier_corrected_is_corrected_both_ways() {
    let w = run("updated_by_err");
    assert_eq!(w.updated_by[0].word, UpdateWord::Err);
    let v = view_of(&w);
    assert!(v.status.corrected);
    assert_eq!(v.genre, Some(Genre::Article));
    let target = doi("10.1016/j.jcma.2013.08.012");
    assert_eq!(
        relations(&w),
        [
            Related {
                kind: Kind::Erratum,
                target: target.clone()
            },
            Related {
                kind: Kind::UpdatedBy(Update::Erratum),
                target
            }
        ]
    );
}

/// 10.1039/c3sm51720a, shard 24040 line 1594: an RSC "Comment on …",
/// with a `print` update to itself both ways — when the print version
/// appeared, which is no update, set aside and counted.
#[test]
fn a_comment_is_kept_and_print_is_set_aside() {
    let out = collect("update_to_comment_and_print");
    assert_eq!(
        out.set_aside,
        [("update-to", "print"), ("updated-by", "print")]
    );
    let w = &out.works[0];
    assert_eq!(w.updates_to.len(), 1);
    assert!(w.updated_by.is_empty());
    assert_eq!(view::genre(w), Some(Genre::Comment));
    assert_eq!(
        relations(w),
        [Related {
            kind: Kind::IsCommentOn,
            target: doi("10.1039/c0sm00236d")
        }]
    );
}

/// 10.1107/s0907444907050196, shard 30645 line 2751: `IUCr`, in a dispute:
/// it comments on one work, a third comments on it, and that comment is
/// copied into its `relation` under the word.
#[test]
fn a_comment_either_way_and_in_relation() {
    let w = run("comment_in_relation");
    let kinds: Vec<_> = relations(&w).into_iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        [Kind::HasComment, Kind::IsCommentOn, Kind::HasComment]
    );
}

/// 10.1017/s0950268813002343, shard 24028 line 1714: Cambridge's
/// `corrected-article`, labelled "Corrected Article", on a one-page notice
/// titled "… – CORRIGENDUM": the label names the work it corrects.
#[test]
fn cambridge_corrected_article_is_a_correction_notice() {
    let w = run("update_to_corrected_article");
    let u = &w.updates_to[0];
    assert_eq!(
        (u.kind, u.word),
        (UpdateType::Correction, UpdateWord::CorrectedArticle)
    );
    assert_eq!(view::genre(&w), Some(Genre::Erratum));
}

/// 10.1103/physrevaccelbeams.21.011003, shard 17883 line 3072: APS's
/// `corrected`, an update from the article to itself: corrected in place,
/// and still the article, not a notice.
#[test]
fn an_article_corrected_in_place_is_corrected_and_still_an_article() {
    let w = run("corrected_in_place");
    assert_eq!(w.updates_to[0].doi, w.doi);
    let v = view_of(&w);
    assert!(v.status.corrected);
    assert_eq!(v.genre, Some(Genre::Article));
}

/// 10.3410/f1000devtestarticled, shard 25797 line 3373: F1000's test
/// deposit, `this_is_some_update_23`: kept out by the exclusion list.
#[test]
fn a_test_deposit_is_kept_out() {
    let out = collect("test_deposit");
    assert!(out.works.is_empty() && out.references.is_empty());
    assert_eq!(out.excluded, ["a test deposit"]);
}

/// 10.1149/2162-8777/ac4c7b, shard 10369 line 1514: the ECS cites a
/// patent; `patent` stopped 1,070 shards.
#[test]
fn a_reference_can_be_to_a_patent() {
    let native = native("reference_patent");
    let refs = native.reference.as_deref().unwrap();
    assert!(refs.iter().any(|r| r.r#type == Some(ReferenceType::Patent)));
    let out = collect("reference_patent");
    assert_eq!(out.references.len(), refs.len());
}

/// 10.20944/preprints202303.0163.v1, shard 8057 line 3110: Preprints.org
/// names the published version by `short-doi`, which is a whole DOI.
#[test]
fn a_short_doi_is_read_as_a_doi() {
    let w = run("relation_short_doi");
    assert_eq!(w.relations[0].id_kind, RelationIdKind::ShortDoi);
    assert_eq!(
        relations(&w),
        [Related {
            kind: Kind::IsPreprintOf,
            target: doi("10.46827/ejmms.v8i2.1458")
        }]
    );
}

/// 10.1016/j.catena.2025.109263, shard 2397 line 418: an ORCID as what
/// the article `is-supplemented-by`, the publisher's mistake: kept, and
/// no name.
#[test]
fn an_orcid_relation_is_kept_and_names_nothing() {
    let w = run("relation_orcid");
    let r = w
        .relations
        .iter()
        .find(|r| r.id_kind == RelationIdKind::Orcid)
        .unwrap();
    assert_eq!(r.id, "0009-0006-9864-1886");
    let orcid = relations(&w)
        .into_iter()
        .find(|r| r.kind == Kind::IsSupplementedBy)
        .unwrap();
    assert_eq!(orcid.target, None);
}

/// 10.46936/cpcy.proj.2021.60233/60007233, shard 10886 line 4911: a DOE
/// grant funded as an `award`, one of the grant schema's 18 kinds.
#[test]
fn a_grant_can_be_funded_as_an_award() {
    let native = native("funding_award");
    let funding = native.project.as_deref().unwrap()[0]
        .funding
        .as_deref()
        .unwrap();
    assert_eq!(funding[0].r#type, Some(FundingType::Award));
    run("funding_award");
}

/// 10.26599/hyd.2025.9380007.v1.rew1, shard 1656 line 3321: Tsinghua
/// University Press's review recommends `accept-with-reservation`.
#[test]
fn a_review_can_accept_with_reservation() {
    let native = native("review_accept_with_reservation");
    assert_eq!(
        native.review.as_ref().unwrap().recommendation,
        Some(ReviewRecommendation::AcceptWithReservation)
    );
    run("review_accept_with_reservation");
}
