//! What the merge asks of a PubMed record beyond its names (merge plan
//! §5, §6): the view of its promoted fields, and the tables from
//! PubMed's vocabularies into the merge's — publication types to a
//! genre, `CommentsCorrections` types to a relation kind.
//!
//! The genre table is keyed by descriptor UI, which is stable where the
//! term is not, and holds the publication types the corpus has shown: the
//! sample's 61 plus the base type (2026-09-15), the update files' few
//! more, and the nine the survey of the whole 2026 baseline, ~37M
//! records, turned up (2026-09-23). A type outside it stops the run,
//! which is how the rest of NLM's list gets added, one evidence at a
//! time.

use model::date::Bound;
use model::symbol::Interned;
use model::{Claim, Table};

use merge::kind::{Form, Genre};
use merge::placement::{Placement, Within};
use merge::relation::Kind;
use merge::view::{Status, Text, Unmapped, Venue, View, bound, join_sections, strip_markup, text};

use crate::attr::RefType;
use crate::normalize::{AuthorName, Citation, PersonName, Published};

/// What scopes the record's publisher ids (merge plan §3.1): every DOI it
/// holds, because either DOI slot may carry the registered one and a
/// record whose slots disagree still keys its publisher ids by one
/// registrant, the lower.
#[must_use]
pub fn publisher_scope<'w>(work: &'w Citation<'_>) -> merge::publisher::Scope<'w> {
    let mut scope = merge::publisher::Scope::new();
    if let Some(doi) = work.ids.doi.as_ref().and_then(Claim::valid) {
        scope.doi(doi);
    }
    for doi in work.ids.elocation_doi.iter().filter_map(Claim::valid) {
        scope.doi(doi);
    }
    scope
}

/// The publisher ids the record carries, each with the normalized field
/// it came from: `ArticleId IdType="pii"` and `ELocationID EIdType="pii"`,
/// which need not agree and are both kept (model version 2).
pub fn publisher_ids<'w>(work: &'w Citation<'_>) -> impl Iterator<Item = (&'static str, &'w str)> {
    let pii = work.ids.pii.iter().map(|v| ("ids.pii", *v));
    let elocation = work
        .ids
        .elocation_pii
        .iter()
        .map(|v| ("ids.elocation_pii", *v));
    pii.chain(elocation)
}

/// A PubMed publication type's place in the genre table: a genre with a
/// priority (higher wins among a record's types), or a qualifier that
/// says nothing about the genre — a study design, a funding note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Genre(Genre, u8),
    Qualifier,
}

/// Retracted Publication: the record was retracted (status, not genre).
const RETRACTED_PUBLICATION: &str = "D016441";

/// Duplicate Publication: the record is a duplicate of material already
/// published elsewhere, unacknowledged (MeSH D016438) — status, not
/// genre, and so a qualifier in the table as well as a flag here.
///
/// Unlike `retracted`, there is nothing to OR this with. PubMed's whole
/// relation vocabulary was read from the raw XML (2026-09-23) and has no
/// `Duplicate*` `RefType`: the 23 words are the ones [`relation_kind`]
/// spells, and none of them says "is a duplicate of". So the publication
/// type is the only evidence, and the flag it sets says only that this
/// record is a duplicate — never of what. Finding the other copy is the
/// merge's own work, which is why it wants the fact.
const DUPLICATE_PUBLICATION: &str = "D016438";

/// Where a publication type sits in the table, as one word: the genre it
/// gives a record, `qualifier` for the types that say nothing about genre,
/// or `None` when the table has never seen it — which is what stops a merge
/// (`ol-rht pubmed genres`, so the corpus can be asked before it is merged
/// rather than after).
#[must_use]
pub fn genre_of(ui: &str) -> Option<&'static str> {
    Some(match place(ui)? {
        Place::Genre(genre, _) => genre.name(),
        Place::Qualifier => "qualifier",
    })
}

/// The table, by descriptor UI. Priorities: a notice about another work
/// (retraction, erratum, concern) over the base a record cannot be
/// without (article); a review over the article it also is; and, between
/// those, a word the other sources also speak over one only NLM has,
/// because the promoted genre is compared across sources and a word
/// Crossref and OpenAlex cannot say makes nothing agree or disagree
/// (2026-09-23).
#[allow(clippy::too_many_lines)]
fn place(ui: &str) -> Option<Place> {
    Some(match ui {
        // Retraction Notice; Published Erratum; Expression of Concern.
        "D016440" => Place::Genre(Genre::Retraction, 90),
        "D016425" => Place::Genre(Genre::Erratum, 90),
        "D000075742" => Place::Genre(Genre::ExpressionOfConcern, 90),
        // Scientific Integrity Review, which MeSH scopes as the U.S.
        // Office of Research Integrity's reports naming questionable
        // research: a notice about other work, like the 90s, not a form.
        // Below them because when a record is both, the journal's own
        // retraction or erratum is the act and this is the finding that
        // prompted it.
        "D016426" => Place::Genre(Genre::IntegrityReview, 85),
        // Preprint; Dataset.
        "D000076942" => Place::Genre(Genre::Preprint, 80),
        "D064886" => Place::Genre(Genre::Dataset, 80),
        // Editorial; Introductory Journal Article.
        "D016421" | "D054711" => Place::Genre(Genre::Editorial, 70),
        // Letter.
        "D016422" => Place::Genre(Genre::Letter, 70),
        // Comment.
        "D016420" => Place::Genre(Genre::Comment, 65),
        // News; Newspaper Article.
        "D016433" | "D018431" => Place::Genre(Genre::News, 60),
        // Review; Systematic Review; Meta-Analysis; Scoping Review;
        // Network Meta-Analysis.
        "D016454" | "D000078182" | "D017418" | "D000098583" | "D000099094" => {
            Place::Genre(Genre::Review, 50)
        }
        // Practice Guideline; Guideline; Consensus Statement; Consensus
        // Development Conference, NIH.
        "D017065" | "D016431" | "D016446" | "D016447" => Place::Genre(Genre::Guideline, 45),
        // Congress, which also spells Conference Proceedings; Clinical
        // Conference (pubmed26n1337).
        "D016423" | "D016429" => Place::Genre(Genre::ConferencePaper, 40),
        // Technical Report, which was this genre's only member, joined by
        // Government Publication: MeSH scopes it as anything a local,
        // regional or national government or its agencies issues, which
        // is a technical report under another name (2026-09-23).
        "D016427" | "D022903" => Place::Genre(Genre::Report, 40),
        // The six forms NLM indexes that no aggregator names. All at one
        // priority: each is a definite statement about what the document
        // is, so above `Other`'s 30, which is the shrug; below
        // `ConferencePaper` and `Report`'s 40, which say the same kind of
        // thing in a word the other sources can also say. They do not
        // compete with each other — nothing in the corpus is both a
        // dictionary and a webcast — so a band rather than an order.
        //
        // Festschrift: a collection honouring a person or an institution,
        // which is what the volume is, whatever its articles are about.
        "D016221" => Place::Genre(Genre::Festschrift, 35),
        // Legislation, the text of a bill or a statute; Legal Case, a
        // court's decision. Both are the law itself rather than writing
        // about it, which is what sets them apart from an article on
        // health policy.
        "D020485" | "D016418" => Place::Genre(Genre::Legal, 35),
        // Webcast; Video-Audio Media. A recording, not a text: the thing
        // a reader cannot read.
        "D057405" | "D059040" => Place::Genre(Genre::Media, 35),
        // Dictionary; Bibliography; Directory; Periodical Index. Works to
        // be consulted rather than read through — they answer a lookup,
        // and none of them argues anything.
        "D016437" | "D016417" | "D016435" | "D016453" => Place::Genre(Genre::Reference, 35),
        // Interactive Tutorial; Patient Education Handout. Written to
        // teach someone, the reader named in the form itself.
        "D054710" | "D029282" => Place::Genre(Genre::Instructional, 35),
        // What is left of the grab-bag after the six forms above took
        // their share (2026-09-23): Interview; Lecture; Address;
        // Biography; Portrait; Personal Narrative; Autobiography; and
        // Overall, the summary record of a set of articles. Each names a
        // way of speaking rather than a kind of document, and the genres
        // the merge has do not tell them apart — a lecture and an
        // interview are both, as far as the vocabulary goes, an article
        // that is not one.
        "D017203" | "D019531" | "D019484" | "D019215" | "D019477" | "D062210" | "D016424"
        | "D020493" => Place::Genre(Genre::Other, 30),
        // Journal Article; Case Reports; Historical Article; Corrected and
        // Republished Article; Clinical Trial Protocol.
        "D016428" | "D002363" | "D016456" | "D016439" | "D000078325" => {
            Place::Genre(Genre::Article, 10)
        }
        // Retracted Publication and Duplicate Publication are status,
        // not genre. Seminal Article is a judgement of importance —
        // MeSH scopes it as a milestone frequently cited since — and
        // says nothing about the document's form; the rest are study
        // designs and funding notes.
        RETRACTED_PUBLICATION
        | DUPLICATE_PUBLICATION
        | "D016419"
        | "D013485"
        | "D052061"
        | "D013486"
        | "D013487"
        | "D052060"
        | "D057666"
        | "D003160"
        | "D016449"
        | "D016448"
        | "D064888"
        | "D004740"
        | "D016430"
        | "D023361"
        | "D023362"
        | "D017428"
        | "D017427"
        | "D017426"
        | "D017429"
        | "D018848"
        | "D000073843"
        | "D065007"
        | "D000068397"
        | "D018486"
        | "D000081262"
        | "D000077522"
        // Observational Study, Veterinary; Adaptive Clinical Trial
        // (pubmed26n1335 to 1339).
        | "D000078323"
        | "D000076362" => Place::Qualifier,
        _ => return None,
    })
}

/// A record's genre: the highest-priority type of its publication types.
///
/// # Errors
///
/// A publication type outside the table.
fn genre(work: &Citation<'_>) -> Result<Option<Genre>, Unmapped> {
    let mut best: Option<(u8, Genre)> = None;
    for t in &work.publication_types {
        let ui = t.ui.as_str();
        match place(ui) {
            Some(Place::Genre(g, priority)) => {
                if best.is_none_or(|(p, _)| priority > p) {
                    best = Some((priority, g));
                }
            }
            Some(Place::Qualifier) => {}
            None => {
                return Err(Unmapped {
                    field: "publication_types",
                    value: format!("{ui} ({})", t.name.as_text().unwrap_or("?")),
                });
            }
        }
    }
    Ok(best.map(|(_, g)| g))
}

/// The form PubMed implies: a journal article, unless the record says
/// preprint or dataset.
fn form(genre: Option<Genre>) -> Form {
    match genre {
        Some(Genre::Preprint) => Form::PostedContent,
        Some(Genre::Dataset) => Form::Dataset,
        _ => Form::JournalArticle,
    }
}

/// The view of one record.
///
/// # Errors
///
/// A publication type outside the table, or a symbol beyond the set's
/// table.
pub fn view<'w>(work: &'w Citation<'_>, table: &'w Table) -> Result<View<'w>, Unmapped> {
    let genre = genre(work)?;
    // A date kept rather than read compares as what of it is certain: its
    // year, or a backwards day range's first day (model version 7).
    let issue = match &work.dates.published {
        Published::Read(date) | Published::Kept { certain: date, .. } => bound(date),
    };
    let electronic = work.dates.electronic.map(|d| Bound {
        year: d.year(),
        month: Some(d.month()),
        day: Some(d.day()),
    });
    let published = match electronic {
        Some(e) if merge::view::earlier(&e, &issue).is_lt() => e,
        _ => issue,
    };
    let mut issns = smallvec::SmallVec::new();
    for claim in work
        .journal
        .issn
        .as_ref()
        .map(|t| &t.issn)
        .into_iter()
        .chain(work.journal.issn_linking.as_ref())
    {
        if let Claim::Valid(issn) = claim
            && !issns.contains(issn)
        {
            issns.push(*issn);
        }
    }
    let nlm = match &work.journal.nlm_unique_id {
        Some(id) => Some(Interned::Text(text(id, table, "journal.nlm_unique_id")?)),
        None => None,
    };
    let retracted = work
        .publication_types
        .iter()
        .any(|t| t.ui.as_str() == RETRACTED_PUBLICATION)
        || work.relations.iter().any(|r| {
            matches!(
                r.kind,
                RefType::RetractionIn | RefType::RetractedAndRepublishedIn
            )
        });
    let corrected = work.relations.iter().any(|r| {
        matches!(
            r.kind,
            RefType::ErratumIn | RefType::CorrectedAndRepublishedIn
        )
    });
    let concern = work
        .relations
        .iter()
        .any(|r| r.kind == RefType::ExpressionOfConcernIn);
    // No relation to OR in: see `DUPLICATE_PUBLICATION`.
    let duplicate = work
        .publication_types
        .iter()
        .any(|t| t.ui.as_str() == DUPLICATE_PUBLICATION);
    Ok(View {
        version: work.version,
        set: "",
        superseded: Vec::new(),
        revised: work.dates.revised,
        titles: <crate::Pubmed as merge::Contributor>::titles(work),
        has_abstract: work.r#abstract.is_some() || !work.other_abstracts.is_empty(),
        authors: u32::try_from(work.authors.len()).unwrap_or(u32::MAX),
        // The issue's date is the citation's (D4; CF2's rule 5).
        placement: Placement::of(
            Venue {
                openalex: None,
                issns,
                nlm,
            },
            work.journal.volume.as_ref(),
            work.journal.issue.as_ref(),
            work.journal.level_in_pages,
            work.journal.pages.as_ref().map(Within::of_pages),
            Some(issue),
        ),
        published: Some(published),
        online: electronic,
        language: work.languages.first().copied(),
        form: Some(form(genre)),
        genre,
        status: Status {
            retracted,
            corrected,
            concern,
            duplicate,
        },
        access: work.ids.pmc.as_ref().and_then(Claim::valid).map(|_| true),
        references: work.number_of_references,
        cited_by: None,
    })
}

/// A `CommentsCorrections` type as the merge's relation kind: the same
/// 23 words.
#[must_use]
pub const fn relation_kind(kind: RefType) -> Kind {
    match kind {
        RefType::AssociatedDataset => Kind::AssociatedDataset,
        RefType::AssociatedPublication => Kind::AssociatedPublication,
        RefType::CommentIn => Kind::CommentIn,
        RefType::CommentOn => Kind::CommentOn,
        RefType::CorrectedAndRepublishedIn => Kind::CorrectedAndRepublishedIn,
        RefType::CorrectedAndRepublishedFrom => Kind::CorrectedAndRepublishedFrom,
        RefType::ErratumIn => Kind::ErratumIn,
        RefType::ErratumFor => Kind::ErratumFor,
        RefType::ExpressionOfConcernIn => Kind::ExpressionOfConcernIn,
        RefType::ExpressionOfConcernFor => Kind::ExpressionOfConcernFor,
        RefType::RepublishedIn => Kind::RepublishedIn,
        RefType::RepublishedFrom => Kind::RepublishedFrom,
        RefType::RetractedAndRepublishedIn => Kind::RetractedAndRepublishedIn,
        RefType::RetractedAndRepublishedFrom => Kind::RetractedAndRepublishedFrom,
        RefType::RetractionIn => Kind::RetractionIn,
        RefType::RetractionOf => Kind::RetractionOf,
        RefType::UpdateIn => Kind::UpdateIn,
        RefType::UpdateOf => Kind::UpdateOf,
        RefType::SummaryForPatientsIn => Kind::SummaryForPatientsIn,
        RefType::OriginalReportIn => Kind::OriginalReportIn,
        RefType::ReprintIn => Kind::ReprintIn,
        RefType::ReprintOf => Kind::ReprintOf,
        RefType::Cites => Kind::Cites,
    }
}

/// The record's text for the serving layer: the abstract's sections
/// joined under their labels, one display name per author, the journal's
/// title or, failing that, its Medline abbreviation.
///
/// # Errors
///
/// A symbol beyond the set's table.
pub fn text_of(work: &Citation<'_>, table: &Table) -> Result<Text, Unmapped> {
    let abstract_text = work
        .r#abstract
        .as_ref()
        .map(|a| join_sections(a.sections.iter().map(|s| (s.label, s.text))))
        .filter(|t| !t.is_empty());
    let authors = work
        .authors
        .iter()
        .map(|a| match &a.name {
            AuthorName::Person(p) => person(p),
            AuthorName::Collective { name, .. } => strip_markup(name),
        })
        .collect();
    let venue = match &work.journal.title {
        Some(title) => text(title, table, "journal.title")?,
        None => text(&work.journal.medline_ta, table, "journal.medline_ta")?,
    };
    Ok(Text {
        abstract_text,
        authors,
        venue: Some(venue.to_owned()).filter(|v| !v.is_empty()),
    })
}

/// `ForeName LastName`, or the initials when the forename is missing, and
/// the suffix after.
fn person(p: &PersonName<'_>) -> String {
    let mut name = String::new();
    if let Some(given) = p.fore.or(p.initials) {
        name.push_str(given);
        name.push(' ');
    }
    name.push_str(p.last);
    if let Some(suffix) = p.suffix {
        name.push(' ');
        name.push_str(&strip_markup(suffix));
    }
    name
}

#[cfg(test)]
mod tests {
    use merge::kind::Genre;

    use super::{DUPLICATE_PUBLICATION, Place, genre_of, place};

    /// What `ol-rht pubmed genres` answers with: the table asked rather than
    /// printed, so a corpus can be checked before it is merged.
    #[test]
    fn genre_of_names_the_place_or_nothing() {
        assert_eq!(genre_of("D016428"), Some("article"));
        assert_eq!(genre_of("D016454"), Some("review"));
        // A study design says nothing about genre.
        assert_eq!(genre_of("D013485"), Some("qualifier"));
        // The two the oldest baseline files added, 2026-09-23. Directory
        // is a reference work; Autobiography stayed in the grab-bag.
        assert_eq!(genre_of("D016435"), Some("reference"));
        assert_eq!(genre_of("D020493"), Some("other"));
        // Seminal Article and Duplicate Publication are judgements about
        // a work, not statements about what it is.
        assert_eq!(genre_of("D016419"), Some("qualifier"));
        assert_eq!(genre_of(DUPLICATE_PUBLICATION), Some("qualifier"));
        // Not a publication type at all, and not in the table.
        assert_eq!(genre_of("D999999"), None);
        assert_eq!(genre_of(""), None);
    }

    #[test]
    fn the_sample_publication_types_are_all_in_the_table() {
        let observed = [
            "D016428",
            "D016454",
            "D013485",
            "D052061",
            "D002363",
            "D000078182",
            "D016422",
            "D016421",
            "D003160",
            "D013486",
            "D016449",
            "D016425",
            "D016448",
            "D017418",
            "D013487",
            "D064888",
            "D004740",
            "D016420",
            "D016430",
            "D023361",
            "D052060",
            "D016433",
            "D023362",
            "D017428",
            "D000098583",
            "D017065",
            "D016456",
            "D059040",
            "D016446",
            "D016440",
            "D054711",
            "D017427",
            "D000078325",
            "D017426",
            "D000076942",
            "D018848",
            "D000099094",
            "D000073843",
            "D016431",
            "D065007",
            "D000075742",
            "D016441",
            "D000068397",
            "D016423",
            "D018486",
            "D017203",
            "D064886",
            "D017429",
            "D062210",
            "D000081262",
            "D057666",
            "D019531",
            "D000077522",
            "D016427",
            "D016417",
            "D016447",
            "D016439",
            "D019484",
            "D019215",
            "D019477",
            // The update files pubmed26n1335 to 1339.
            "D029282",
            "D016424",
            "D016418",
            "D000078323",
            "D000076362",
            "D016453",
            "D016429",
        ];
        for ui in observed {
            assert!(place(ui).is_some(), "{ui}");
        }
        assert_eq!(place("D999999"), None, "outside the table");
        assert_eq!(place("D016454"), Some(Place::Genre(Genre::Review, 50)));
        assert_eq!(place("D013485"), Some(Place::Qualifier));
    }

    /// The nine the survey of the whole 2026 baseline turned up
    /// (2026-09-23), each with the name the corpus and MeSH both give it,
    /// and the four the `other` bucket gave up to the new genres. A merge
    /// stops on any of these until they are placed, so the test is the
    /// list itself.
    #[test]
    fn the_baseline_survey_types_are_all_placed() {
        let expected = [
            // The nine the survey found, most common first.
            ("D016419", "Seminal Article", Place::Qualifier),
            (
                "D016221",
                "Festschrift",
                Place::Genre(Genre::Festschrift, 35),
            ),
            ("D020485", "Legislation", Place::Genre(Genre::Legal, 35)),
            ("D016438", "Duplicate Publication", Place::Qualifier),
            ("D057405", "Webcast", Place::Genre(Genre::Media, 35)),
            ("D016437", "Dictionary", Place::Genre(Genre::Reference, 35)),
            (
                "D016426",
                "Scientific Integrity Review",
                Place::Genre(Genre::IntegrityReview, 85),
            ),
            (
                "D054710",
                "Interactive Tutorial",
                Place::Genre(Genre::Instructional, 35),
            ),
            (
                "D022903",
                "Government Publication",
                Place::Genre(Genre::Report, 40),
            ),
            // The four that left `other` for a genre that names them.
            (
                "D016417",
                "Bibliography",
                Place::Genre(Genre::Reference, 35),
            ),
            ("D016435", "Directory", Place::Genre(Genre::Reference, 35)),
            (
                "D016453",
                "Periodical Index",
                Place::Genre(Genre::Reference, 35),
            ),
            (
                "D059040",
                "Video-Audio Media",
                Place::Genre(Genre::Media, 35),
            ),
            (
                "D029282",
                "Patient Education Handout",
                Place::Genre(Genre::Instructional, 35),
            ),
            ("D016418", "Legal Case", Place::Genre(Genre::Legal, 35)),
            // And the ones that stayed, which the genres cannot tell apart.
            ("D017203", "Interview", Place::Genre(Genre::Other, 30)),
            ("D019531", "Lecture", Place::Genre(Genre::Other, 30)),
            ("D019484", "Address", Place::Genre(Genre::Other, 30)),
            ("D019215", "Biography", Place::Genre(Genre::Other, 30)),
            ("D019477", "Portrait", Place::Genre(Genre::Other, 30)),
            (
                "D062210",
                "Personal Narrative",
                Place::Genre(Genre::Other, 30),
            ),
            ("D020493", "Autobiography", Place::Genre(Genre::Other, 30)),
            ("D016424", "Overall", Place::Genre(Genre::Other, 30)),
        ];
        for (ui, name, want) in expected {
            assert_eq!(place(ui), Some(want), "{ui} {name}");
        }
    }

    /// The rule the priorities exist for: the highest wins. The six new
    /// genres beat the grab-bag and the base type, and lose to the words
    /// the other sources also speak.
    #[test]
    fn a_records_types_are_settled_by_priority() {
        let best = |uis: &[&str]| {
            uis.iter()
                .filter_map(|ui| match place(ui) {
                    Some(Place::Genre(g, p)) => Some((p, g)),
                    _ => None,
                })
                .max_by_key(|(p, _)| *p)
                .map(|(_, g)| g)
        };
        // Journal Article + Dictionary: a dictionary, not an article.
        assert_eq!(best(&["D016428", "D016437"]), Some(Genre::Reference));
        // Interview + Video-Audio Media: the recording beats the shrug.
        assert_eq!(best(&["D017203", "D059040"]), Some(Genre::Media));
        // Congress + Festschrift: the word Crossref and OpenAlex also
        // have wins.
        assert_eq!(best(&["D016423", "D016221"]), Some(Genre::ConferencePaper));
        // Journal Article + Scientific Integrity Review: the notice.
        assert_eq!(best(&["D016428", "D016426"]), Some(Genre::IntegrityReview));
        // ... but a retraction notice is the act, and outranks it.
        assert_eq!(best(&["D016426", "D016440"]), Some(Genre::Retraction));
        // Journal Article + Seminal Article: a qualifier adds nothing.
        assert_eq!(best(&["D016428", "D016419"]), Some(Genre::Article));
        // No types at all: no genre, which is a shape the merge has.
        assert_eq!(best(&[]), None);
    }
}
