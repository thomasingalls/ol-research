//! Records whose lists say one thing twice, or two things at once.
//!
//! `ArticleIdList` is `(ArticleId+)` and `Author` carries `Identifier*`:
//! the DTD permits a kind to repeat, and the survey of the whole 2026
//! baseline, ~37M records (2026-09-23), found that PubMed does it — on 16
//! references, 59 authors and one record's own id list. Every one of them
//! used to stop the run. These are those records, re-fetched from `efetch`
//! on 2026-09-23 with the repetition still in them, one per shape, and the
//! shapes that are still refused.

use ingest::Sink;
use model::{Claim, PmcId, Pmid};
use source_pubmed::attr::IdType;
use source_pubmed::normalize::{Citation, Error, Reference, normalize};
use source_pubmed::record::Record;
use source_pubmed::{ArticleSet, Pubmed};

fn fixture(name: &str) -> String {
    let path = format!("{}/fixtures/{name}.xml", env!("CARGO_MANIFEST_DIR"));
    let body = std::fs::read_to_string(&path).expect("fixture exists");
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<PubmedArticleSet>\n{body}</PubmedArticleSet>\n"
    )
}

#[derive(Debug, Default)]
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

/// One record, normalized, or the value that stopped it.
fn read(text: String) -> Result<Collect<'static>, Error> {
    let input = Box::leak(text.into_boxed_str());
    let mut set = ArticleSet::new(input);
    let Some(Record::Article(article)) = set.next_record().expect("parses") else {
        panic!("expected one article")
    };
    let article = Box::leak(Box::new(article));
    let mut out = Collect::default();
    normalize(article, &mut out)?;
    Ok(out)
}

fn run(name: &str) -> Collect<'static> {
    read(fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Every DOI the record's references named, in order.
fn reference_dois(out: &Collect<'_>) -> Vec<String> {
    out.references
        .iter()
        .filter_map(|r| r.doi.as_ref().and_then(Claim::valid))
        .map(|d| d.as_str().to_owned())
        .collect()
}

/// PMID 28287839: its second author gives one ORCID twice, and its third
/// gives her own and then the second author's. The first is one assertion
/// made twice and is kept; the second is two, and the record says neither —
/// naming the wrong person is worse than naming none. Everything else about
/// both authors survives, which is the point: the record used to be lost.
#[test]
fn an_author_who_says_one_orcid_twice_keeps_it_and_one_who_says_two_keeps_neither() {
    let c = &run("orcid_twice").citations[0];
    let said_it_twice = &c.authors[1];
    let said_two = &c.authors[2];
    assert_eq!(
        said_it_twice
            .orcid
            .as_ref()
            .and_then(Claim::valid)
            .map(ToString::to_string),
        Some("0000-0003-0217-4269".to_owned())
    );
    assert!(said_two.orcid.is_none(), "{:?}", said_two.orcid);
    assert!(
        !said_two.affiliations.is_empty(),
        "the author is still here"
    );
}

/// PMID 32796281: one reference gives `10.1212/WNL.0000000000010155.`
/// twice, the same string both times. One DOI, asserted twice — including
/// the full stop, which this is not the place to second-guess when both
/// assertions carry it.
#[test]
fn a_reference_that_gives_one_id_twice_keeps_it() {
    let out = run("ref_doi_twice");
    let dois = reference_dois(&out);
    assert!(
        dois.contains(&"10.1212/wnl.0000000000010155.".to_owned()),
        "{dois:?}"
    );
    assert!(
        out.references.iter().all(|r| r.loose.is_empty()),
        "nothing was set aside"
    );
}

/// PMID 32925547: two of its references give a DOI twice, once with the
/// citation sentence's full stop attached (`10.1002/jmv.26224.` beside
/// `10.1002/jmv.26224`). The bare form is the one both assertions share —
/// and the one `doi.org` resolves; the dotted form 404s — so the reference
/// keeps its id and its edge.
#[test]
fn a_reference_id_drops_the_citation_s_full_stop_when_the_bare_form_is_there_too() {
    let out = run("ref_doi_punctuated");
    let dois = reference_dois(&out);
    for want in ["10.1002/jmv.26224", "10.1002/art.41454"] {
        assert!(dois.contains(&want.to_owned()), "{want} in {dois:?}");
    }
    assert!(
        !dois.iter().any(|d| d.ends_with('.')),
        "the punctuated forms are gone: {dois:?}"
    );
    assert!(out.references.iter().all(|r| r.loose.is_empty()));
}

/// PMID 32956132: one reference to a Cochrane review gives
/// `10.1002/14651858.CD100105.` and `10.1002/14651858.CD000313.pub5` — two
/// different DOIs, only the second of which is registered, and nothing in
/// the record says which is meant. A reference names one work and the merge
/// unions every name it carries, so naming both would make one work of two;
/// the slot stays empty and both values go to `loose`, which the matching
/// stage keeps and which is never a name.
#[test]
fn a_reference_whose_ids_disagree_names_neither_and_keeps_both() {
    let out = run("ref_doi_disagrees");
    let disagreed: Vec<&Reference<'_>> = out
        .references
        .iter()
        .filter(|r| !r.loose.is_empty())
        .collect();
    assert_eq!(disagreed.len(), 1);
    let r = disagreed[0];
    assert!(r.doi.is_none(), "{:?}", r.doi);
    let loose: Vec<&str> = r.loose.iter().map(|l| l.value).collect();
    assert_eq!(
        loose,
        [
            "10.1002/14651858.CD100105.",
            "10.1002/14651858.CD000313.pub5"
        ]
    );
    assert!(r.loose.iter().all(|l| l.kind == IdType::Doi));
    assert!(!r.citation.is_empty(), "the citation text is still there");
    assert!(
        reference_dois(&out).contains(&"10.1097/jnn.0000000000000515".to_owned()),
        "its other references are untouched"
    );
}

/// PMID 34621506 (pubmed26n1339) is version 2 of its citation and gives
/// `PMC8459621.2`; the id converter says PMC holds two versions of that
/// article, so the counters agree here by coincidence. The version is read
/// to check it is one and then dropped — `PmcId` names the article.
#[test]
fn a_pmc_id_keeps_its_accession_and_drops_the_version() {
    let c = &run("pmc_version").citations[0];
    assert_eq!(
        c.ids.pmc.as_ref().and_then(Claim::valid),
        Some(&PmcId::new(8_459_621).expect("a PMC id"))
    );
}

/// The same record with the suffix the 2026 baseline's 12 failures show: a
/// PMC version that is not the record's own. `PMID@Version` counts
/// revisions of the citation and the suffix counts versions of the article
/// in PMC — PMID 35903216 is at version 4 with `PMC9277196.3` — so the two
/// need not agree, and requiring it (2026-09-18) only lost the records.
#[test]
fn a_pmc_version_that_is_not_the_records_version_is_read_all_the_same() {
    let text = fixture("pmc_version").replace("PMC8459621.2", "PMC8459621.3");
    let out = read(text).expect("normalizes");
    assert_eq!(
        out.citations[0].ids.pmc.as_ref().and_then(Claim::valid),
        Some(&PmcId::new(8_459_621).expect("a PMC id"))
    );
}

/// What is still refused there: a suffix that is not a version number at
/// all. No record has shown one, and a `pmc` slot this build cannot read is
/// not one to guess at.
#[test]
fn a_pmc_suffix_that_is_not_a_number_still_stops_the_run() {
    let text = fixture("pmc_version").replace("PMC8459621.2", "PMC8459621.two");
    let err = read(text).expect_err("stops");
    assert_eq!(err.field, "ArticleId[pmc]");
    assert_eq!(err.raw, "PMC8459621.two");
}

/// A record's own list, which repeats a kind on exactly one record of the
/// 2026 baseline: PMID 26417354 gives `10.17179/excli2014-603` and
/// `10.3205/2014-603`, of which only the first is registered. NLM has since
/// dropped the second, so the shape is reproduced here on the smallest
/// fixture. Said twice, the record has said it; said two ways, it has not,
/// and the slot stays empty rather than the record being lost — PMID
/// 26417354 still keeps that DOI, which it also gives as its `ELocationID`.
#[test]
fn a_records_own_id_list_may_repeat_a_kind() {
    const DOI: &str = "<ArticleId IdType=\"doi\">10.1001/jama.2025.15703</ArticleId>";
    let twice = fixture("shortest").replace(DOI, &format!("{DOI}{DOI}"));
    let c = &read(twice).expect("normalizes").citations[0];
    assert_eq!(
        c.ids
            .doi
            .as_ref()
            .and_then(Claim::valid)
            .map(model::Doi::as_str),
        Some("10.1001/jama.2025.15703")
    );

    let two = fixture("shortest").replace(
        DOI,
        &format!("{DOI}<ArticleId IdType=\"doi\">10.3205/2014-603</ArticleId>"),
    );
    let c = &read(two).expect("normalizes").citations[0];
    assert!(c.ids.doi.is_none(), "{:?}", c.ids.doi);
    assert_eq!(c.ids.pii.len(), 1, "the rest of the list is untouched");
}

/// What is still refused there: a record that names itself twice,
/// differently. Every other slot holds something the record says about
/// itself, where silence is an answer; this one says which record it is,
/// and a record this build cannot key is not one to guess at. Nothing in
/// the 2026 baseline does it.
#[test]
fn a_record_that_names_itself_twice_differently_still_stops_the_run() {
    const PMID: &str = "<ArticleId IdType=\"pubmed\">41609757</ArticleId>";
    let text = fixture("shortest").replace(
        PMID,
        &format!("{PMID}<ArticleId IdType=\"pubmed\">41609758</ArticleId>"),
    );
    let err = read(text).expect_err("stops");
    assert_eq!(err.field, "ArticleIdList/ArticleId");
    assert_eq!(err.raw, "41609758");

    let same = fixture("shortest").replace(PMID, &format!("{PMID}{PMID}"));
    assert!(read(same).is_ok(), "the same id twice is one id");
}
