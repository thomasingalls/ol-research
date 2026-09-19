//! What the merge asks of an OpenAlex record beyond its names (merge plan
//! §5, §6): the view of its promoted fields, the table from OpenAlex's
//! work types to the merge's genre and the form each implies, and the
//! locations whose DOI or PMID is another work's as `has-version`
//! relations (§3.1).

use model::Table;
use model::date::Bound;

use merge::kind::{Form, Genre};
use merge::relation::{Kind, Related};
use merge::view::{Status, Text, Unmapped, Venue, View, inverted_index, text};
use merge::{Name, OwnedName};

use crate::normalize::{LocationId, Work};
use crate::vocab::{OaStatus, WorkType};

/// A work type as the merge's genre: the same words, OpenAlex's being
/// the genre vocabulary the merge adopted.
#[must_use]
pub const fn genre(kind: WorkType) -> Genre {
    match kind {
        WorkType::Article => Genre::Article,
        WorkType::BookChapter => Genre::BookChapter,
        WorkType::Dataset => Genre::Dataset,
        WorkType::Preprint => Genre::Preprint,
        WorkType::Dissertation => Genre::Dissertation,
        WorkType::Book => Genre::Book,
        WorkType::Review => Genre::Review,
        WorkType::Paratext => Genre::Paratext,
        WorkType::Libguides => Genre::Libguides,
        WorkType::Letter => Genre::Letter,
        WorkType::Other => Genre::Other,
        WorkType::ReferenceEntry => Genre::ReferenceEntry,
        WorkType::Report => Genre::Report,
        WorkType::Editorial => Genre::Editorial,
        WorkType::PeerReview => Genre::PeerReview,
        WorkType::Erratum => Genre::Erratum,
        WorkType::Standard => Genre::Standard,
        WorkType::Grant => Genre::Grant,
        WorkType::SupplementaryMaterials => Genre::SupplementaryMaterials,
        WorkType::Retraction => Genre::Retraction,
        WorkType::ConferencePaper => Genre::ConferencePaper,
        WorkType::ConferenceAbstract => Genre::ConferenceAbstract,
        WorkType::DataPaper => Genre::DataPaper,
        WorkType::BookReview => Genre::BookReview,
        WorkType::SoftwarePaper => Genre::SoftwarePaper,
    }
}

/// The form a type implies when no registry says — the third choice of
/// §5's precedence: the journal article for everything that is an
/// article's kind of thing, `other` where the type says nothing about
/// the registered form.
#[must_use]
pub const fn form(kind: WorkType) -> Form {
    match kind {
        WorkType::Article
        | WorkType::Review
        | WorkType::Letter
        | WorkType::Editorial
        | WorkType::Erratum
        | WorkType::Retraction
        | WorkType::DataPaper
        | WorkType::SoftwarePaper
        | WorkType::BookReview
        | WorkType::ConferenceAbstract => Form::JournalArticle,
        WorkType::Preprint => Form::PostedContent,
        WorkType::BookChapter => Form::BookChapter,
        WorkType::Book => Form::Book,
        WorkType::Dataset => Form::Dataset,
        WorkType::Dissertation => Form::Dissertation,
        WorkType::Report => Form::Report,
        WorkType::Standard => Form::Standard,
        WorkType::Grant => Form::Grant,
        WorkType::PeerReview => Form::PeerReview,
        WorkType::ReferenceEntry => Form::ReferenceEntry,
        WorkType::ConferencePaper => Form::ProceedingsArticle,
        WorkType::SupplementaryMaterials => Form::Component,
        WorkType::Paratext | WorkType::Libguides | WorkType::Other => Form::Other,
    }
}

/// The view of one record.
///
/// # Errors
///
/// Never yet: nothing the view reads is interned or outside a table;
/// the signature is the trait's.
pub fn view<'w>(work: &'w Work<'_>, _: &'w Table) -> Result<View<'w>, Unmapped> {
    let primary = work.locations.first().and_then(|l| l.source.as_ref());
    let mut issns = smallvec::SmallVec::new();
    if let Some(source) = primary {
        for claim in source.issn_l.iter().chain(&source.issns) {
            if let model::Claim::Valid(issn) = claim
                && !issns.contains(issn)
            {
                issns.push(*issn);
            }
        }
    }
    Ok(View {
        version: 1,
        set: "",
        revised: Some(work.updated.date()),
        titles: <crate::OpenAlex as merge::Contributor>::titles(work),
        has_abstract: work.abstract_inverted_index.is_some(),
        authors: u32::try_from(work.authorships.len()).unwrap_or(u32::MAX),
        venue: Venue {
            openalex: primary.map(|s| s.id),
            issns,
            nlm: None,
        },
        published: work.published.map(|d| Bound {
            year: d.year(),
            month: Some(d.month()),
            day: Some(d.day()),
        }),
        language: work
            .language
            .as_ref()
            .and_then(model::Claim::valid)
            .copied(),
        form: Some(form(work.kind)),
        genre: Some(genre(work.kind)),
        status: Status {
            retracted: work.is_retracted == Some(true),
            corrected: false,
            concern: false,
        },
        access: Some(work.open_access.status != OaStatus::Closed),
        references: Some(work.referenced_works_count),
        cited_by: Some(work.cited_by_count),
    })
}

/// The record's relations: every location whose DOI or PMID is not the
/// work's own is another version of it (merge plan §3.1, §6).
pub fn relations(work: &Work<'_>, out: &mut dyn FnMut(Related)) {
    let own_doi = work.doi.as_ref().and_then(model::Claim::valid);
    for location in &work.locations {
        let target = match &location.id {
            Some(LocationId::Doi(doi)) if own_doi.is_none_or(|own| own != doi) => {
                OwnedName::Doi(doi.as_str().into())
            }
            Some(LocationId::Pmid(pmid)) if work.pmid != Some(*pmid) => OwnedName::Pmid(*pmid),
            _ => continue,
        };
        out(Related {
            kind: Kind::HasVersion,
            target: Some(target),
        });
    }
}

/// The work's own name, for tests.
#[must_use]
pub const fn own_name(work: &Work<'_>) -> Name<'static> {
    Name::OpenAlex(work.id)
}

/// The record's text for the serving layer: the inverted index back into
/// an abstract, each authorship's display name, the primary location's
/// source name or its raw source name.
///
/// # Errors
///
/// An inverted index that does not decode, or a symbol beyond the set's
/// table.
pub fn text_of(work: &Work<'_>, table: &Table) -> Result<Text, Unmapped> {
    let abstract_text = match work.abstract_inverted_index {
        Some(json) => Some(inverted_index(json)?).filter(|t| !t.is_empty()),
        None => None,
    };
    let authors = work
        .authorships
        .iter()
        .map(|a| a.author.name.to_owned())
        .collect();
    let venue = match work.locations.first() {
        Some(location) => match (&location.source, &location.raw_source_name) {
            (Some(source), _) => Some(text(&source.name, table, "locations.source.name")?),
            (None, Some(raw)) => Some(text(raw, table, "locations.raw_source_name")?),
            (None, None) => None,
        },
        None => None,
    };
    Ok(Text {
        abstract_text,
        authors,
        venue: venue.map(str::to_owned).filter(|v| !v.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use merge::kind::{Form, Genre};

    use super::{form, genre};
    use crate::vocab::WorkType;

    #[test]
    fn types_map_to_a_genre_and_a_form_that_admits_it() {
        for kind in [
            WorkType::Article,
            WorkType::BookChapter,
            WorkType::Dataset,
            WorkType::Preprint,
            WorkType::Dissertation,
            WorkType::Book,
            WorkType::Review,
            WorkType::Paratext,
            WorkType::Libguides,
            WorkType::Letter,
            WorkType::Other,
            WorkType::ReferenceEntry,
            WorkType::Report,
            WorkType::Editorial,
            WorkType::PeerReview,
            WorkType::Erratum,
            WorkType::Standard,
            WorkType::Grant,
            WorkType::SupplementaryMaterials,
            WorkType::Retraction,
            WorkType::ConferencePaper,
            WorkType::ConferenceAbstract,
            WorkType::DataPaper,
            WorkType::BookReview,
            WorkType::SoftwarePaper,
        ] {
            assert!(
                form(kind).admits(genre(kind)),
                "{kind:?}: {} does not admit {}",
                form(kind),
                genre(kind)
            );
        }
        assert_eq!(genre(WorkType::Review), Genre::Review);
        assert_eq!(form(WorkType::Preprint), Form::PostedContent);
    }
}
