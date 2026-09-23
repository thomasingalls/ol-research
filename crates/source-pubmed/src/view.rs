//! What the merge asks of a PubMed record beyond its names (merge plan
//! §5, §6): the view of its promoted fields, and the tables from
//! PubMed's vocabularies into the merge's — publication types to a
//! genre, `CommentsCorrections` types to a relation kind.
//!
//! The genre table is keyed by descriptor UI, which is stable where the
//! term is not, and holds exactly the 61 publication types the sample
//! showed (2026-09-15) plus the base type; a type outside it stops the
//! run, which is how the rest of NLM's list gets added, one evidence at
//! a time.

use model::date::Bound;
use model::symbol::Interned;
use model::{Claim, Table};

use merge::kind::{Form, Genre};
use merge::relation::Kind;
use merge::view::{Status, Text, Unmapped, Venue, View, bound, join_sections, strip_markup, text};

use crate::attr::RefType;
use crate::normalize::{AuthorName, Citation, PersonName};

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
/// without (article); a review over the article it also is.
#[allow(clippy::too_many_lines)]
fn place(ui: &str) -> Option<Place> {
    Some(match ui {
        // Retraction Notice; Published Erratum; Expression of Concern.
        "D016440" => Place::Genre(Genre::Retraction, 90),
        "D016425" => Place::Genre(Genre::Erratum, 90),
        "D000075742" => Place::Genre(Genre::ExpressionOfConcern, 90),
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
        // Technical Report.
        "D016427" => Place::Genre(Genre::Report, 40),
        // Interview; Lecture; Address; Biography; Portrait; Personal
        // Narrative; Bibliography; Video-Audio Media; and, first in the
        // update files pubmed26n1335 to 1339, Patient Education Handout,
        // Legal Case, Periodical Index and Overall, the summary record of
        // a set of articles. Directory and Autobiography joined them from
        // the oldest baseline files, pubmed26n0001 and 0002.
        "D017203" | "D019531" | "D019484" | "D019215" | "D019477" | "D062210" | "D016417"
        | "D059040" | "D029282" | "D016418" | "D016453" | "D016424" | "D016435" | "D020493" => {
            Place::Genre(Genre::Other, 30)
        }
        // Journal Article; Case Reports; Historical Article; Corrected and
        // Republished Article; Clinical Trial Protocol.
        "D016428" | "D002363" | "D016456" | "D016439" | "D000078325" => {
            Place::Genre(Genre::Article, 10)
        }
        // Retracted Publication is status, not genre; the rest are study
        // designs and funding notes.
        RETRACTED_PUBLICATION
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
    let issue = bound(&work.dates.published);
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
    Ok(View {
        version: work.version,
        set: "",
        revised: work.dates.revised,
        titles: <crate::Pubmed as merge::Contributor>::titles(work),
        has_abstract: work.r#abstract.is_some() || !work.other_abstracts.is_empty(),
        authors: u32::try_from(work.authors.len()).unwrap_or(u32::MAX),
        venue: Venue {
            openalex: None,
            issns,
            nlm,
        },
        published: Some(published),
        language: work.languages.first().copied(),
        form: Some(form(genre)),
        genre,
        status: Status {
            retracted,
            corrected,
            concern,
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

    use super::{Place, genre_of, place};

    /// What `ol-rht pubmed genres` answers with: the table asked rather than
    /// printed, so a corpus can be checked before it is merged.
    #[test]
    fn genre_of_names_the_place_or_nothing() {
        assert_eq!(genre_of("D016428"), Some("article"));
        assert_eq!(genre_of("D016454"), Some("review"));
        // A study design says nothing about genre.
        assert_eq!(genre_of("D013485"), Some("qualifier"));
        // The two the oldest baseline files added, 2026-09-23.
        assert_eq!(genre_of("D016435"), Some("other"));
        assert_eq!(genre_of("D020493"), Some("other"));
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
}
