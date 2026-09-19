//! What the merge asks of a Crossref record beyond its names (merge plan
//! §5, §6): the view of its promoted fields, and the tables from
//! Crossref's vocabularies into the merge's — the 30 work types to a
//! form and a default genre, the Crossmark update types, the `relation`
//! types.

use model::symbol::Interned;
use model::{Claim, Doi, PmcId, Pmid, Table};

use merge::kind::{Form, Genre};
use merge::record::Citation;
use merge::relation::{Kind, Related, Update};
use merge::view::{Status, Text, Unmapped, Venue, View, bound, strip_markup, text};
use merge::{Name, OwnedName};

use crate::normalize::{AuthorName, Reference, Work};
use crate::vocab::{RelationIdKind, UpdateType, WorkType};

/// A work type as the merge's form: the same 30 words.
#[must_use]
pub const fn form(kind: WorkType) -> Form {
    match kind {
        WorkType::Book => Form::Book,
        WorkType::BookChapter => Form::BookChapter,
        WorkType::BookPart => Form::BookPart,
        WorkType::BookSection => Form::BookSection,
        WorkType::BookSeries => Form::BookSeries,
        WorkType::BookSet => Form::BookSet,
        WorkType::BookTrack => Form::BookTrack,
        WorkType::Component => Form::Component,
        WorkType::Database => Form::Database,
        WorkType::Dataset => Form::Dataset,
        WorkType::Dissertation => Form::Dissertation,
        WorkType::EditedBook => Form::EditedBook,
        WorkType::Grant => Form::Grant,
        WorkType::Journal => Form::Journal,
        WorkType::JournalArticle => Form::JournalArticle,
        WorkType::JournalIssue => Form::JournalIssue,
        WorkType::JournalVolume => Form::JournalVolume,
        WorkType::Monograph => Form::Monograph,
        WorkType::Other => Form::Other,
        WorkType::PeerReview => Form::PeerReview,
        WorkType::PostedContent => Form::PostedContent,
        WorkType::Proceedings => Form::Proceedings,
        WorkType::ProceedingsArticle => Form::ProceedingsArticle,
        WorkType::ProceedingsSeries => Form::ProceedingsSeries,
        WorkType::ReferenceBook => Form::ReferenceBook,
        WorkType::ReferenceEntry => Form::ReferenceEntry,
        WorkType::Report => Form::Report,
        WorkType::ReportComponent => Form::ReportComponent,
        WorkType::ReportSeries => Form::ReportSeries,
        WorkType::Standard => Form::Standard,
    }
}

/// The genre a form implies when nothing more is known — the third
/// choice of §5's precedence — or the one a Crossmark notice asserts:
/// a record whose `update-to` is a retraction is the retraction notice.
#[must_use]
pub fn genre(work: &Work<'_>) -> Genre {
    let notice = work.updates_to.iter().map(|u| u.kind).max();
    match notice {
        Some(UpdateType::Retraction | UpdateType::PartialRetraction | UpdateType::Withdrawal) => {
            return Genre::Retraction;
        }
        Some(UpdateType::Correction | UpdateType::Corrigendum | UpdateType::Erratum) => {
            return Genre::Erratum;
        }
        Some(UpdateType::ExpressionOfConcern) => return Genre::ExpressionOfConcern,
        _ => {}
    }
    match work.kind {
        WorkType::JournalArticle => Genre::Article,
        WorkType::PostedContent => Genre::Preprint,
        WorkType::PeerReview => Genre::PeerReview,
        WorkType::Dataset | WorkType::Database => Genre::Dataset,
        WorkType::Book | WorkType::EditedBook | WorkType::Monograph | WorkType::ReferenceBook => {
            Genre::Book
        }
        WorkType::BookChapter
        | WorkType::BookPart
        | WorkType::BookSection
        | WorkType::BookTrack => Genre::BookChapter,
        WorkType::ReferenceEntry => Genre::ReferenceEntry,
        WorkType::Dissertation => Genre::Dissertation,
        WorkType::Report | WorkType::ReportComponent => Genre::Report,
        WorkType::Standard => Genre::Standard,
        WorkType::Grant => Genre::Grant,
        WorkType::ProceedingsArticle => Genre::ConferencePaper,
        WorkType::Component => Genre::SupplementaryMaterials,
        WorkType::Journal
        | WorkType::JournalIssue
        | WorkType::JournalVolume
        | WorkType::Proceedings
        | WorkType::ProceedingsSeries
        | WorkType::BookSeries
        | WorkType::BookSet
        | WorkType::ReportSeries => Genre::Paratext,
        WorkType::Other => Genre::Other,
    }
}

/// A Crossmark update type as the merge's.
#[must_use]
pub const fn update(kind: UpdateType) -> Update {
    match kind {
        UpdateType::Addendum => Update::Addendum,
        UpdateType::Clarification => Update::Clarification,
        UpdateType::Correction => Update::Correction,
        UpdateType::Corrigendum => Update::Corrigendum,
        UpdateType::Erratum => Update::Erratum,
        UpdateType::ExpressionOfConcern => Update::ExpressionOfConcern,
        UpdateType::NewEdition => Update::NewEdition,
        UpdateType::NewVersion => Update::NewVersion,
        UpdateType::PartialRetraction => Update::PartialRetraction,
        UpdateType::Removal => Update::Removal,
        UpdateType::Retraction => Update::Retraction,
        UpdateType::Withdrawal => Update::Withdrawal,
    }
}

/// A `relation` type as the merge's relation kind: the deposit schema's
/// inter-work relations, two more the data showed, and the Crossmark
/// words publishers deposit here too, in Crossref's own spellings.
#[must_use]
pub fn relation_kind(kind: &str) -> Option<Kind> {
    Some(match kind {
        "has-review" => Kind::HasReview,
        "is-review-of" => Kind::IsReviewOf,
        "has-preprint" => Kind::HasPreprint,
        "is-preprint-of" => Kind::IsPreprintOf,
        "is-supplemented-by" => Kind::IsSupplementedBy,
        "is-supplement-to" => Kind::IsSupplementTo,
        "is-referenced-by" => Kind::IsReferencedBy,
        "references" => Kind::References,
        "is-cited-by" => Kind::IsCitedBy,
        "is-version-of" => Kind::IsVersionOf,
        "has-version" => Kind::HasVersion,
        "is-same-as" => Kind::IsSameAs,
        "is-identical-to" => Kind::IsIdenticalTo,
        "is-component-of" => Kind::IsComponentOf,
        "has-part" => Kind::HasPart,
        "is-part-of" => Kind::IsPartOf,
        "is-translation-of" => Kind::IsTranslationOf,
        "has-translation" => Kind::HasTranslation,
        "documents" => Kind::Documents,
        "is-documented-by" => Kind::IsDocumentedBy,
        "is-basis-for" => Kind::IsBasisFor,
        "is-based-on" => Kind::IsBasedOn,
        "is-financed-by" => Kind::IsFinancedBy,
        "finances" => Kind::Finances,
        "has-derivation" => Kind::HasDerivation,
        "is-derived-from" => Kind::IsDerivedFrom,
        "has-manifestation" => Kind::HasManifestation,
        "is-manifestation-of" => Kind::IsManifestationOf,
        "is-manuscript-of" => Kind::IsManuscriptOf,
        "has-manuscript" => Kind::HasManuscript,
        "is-comment-on" => Kind::IsCommentOn,
        "has-comment" => Kind::HasComment,
        "is-reply-to" => Kind::IsReplyTo,
        "has-reply" => Kind::HasReply,
        "is-original-form-of" => Kind::IsOriginalFormOf,
        "is-variant-form-of" => Kind::IsVariantFormOf,
        "is-expression-of" => Kind::IsExpressionOf,
        "has-expression" => Kind::HasExpression,
        "is-format-of" => Kind::IsFormatOf,
        "has-format" => Kind::HasFormat,
        "is-related-material" => Kind::IsRelatedMaterial,
        "has-related-material" => Kind::HasRelatedMaterial,
        "requires" => Kind::Requires,
        "is-required-by" => Kind::IsRequiredBy,
        "is-continued-by" => Kind::IsContinuedBy,
        "continues" => Kind::Continues,
        "compiles" => Kind::Compiles,
        "is-compiled-by" => Kind::IsCompiledBy,
        "is-replaced-by" => Kind::IsReplacedBy,
        "replaces" => Kind::Replaces,
        "is-data-basis-for" => Kind::IsDataBasisFor,
        "based-on-data" => Kind::BasedOnData,
        "correction" => Kind::Correction,
        "erratum" => Kind::Erratum,
        "corrigendum" => Kind::Corrigendum,
        "new_version" => Kind::NewVersion,
        "expression_of_concern" => Kind::ExpressionOfConcern,
        "addendum" => Kind::Addendum,
        "clarification" => Kind::Clarification,
        "new_edition" => Kind::NewEdition,
        "partial_retraction" => Kind::PartialRetraction,
        "removal" => Kind::Removal,
        "retraction" => Kind::Retraction,
        "withdrawal" => Kind::Withdrawal,
        _ => return None,
    })
}

/// The view of one record.
///
/// # Errors
///
/// A symbol beyond the set's table.
pub fn view<'w>(work: &'w Work<'_>, table: &'w Table) -> Result<View<'w>, Unmapped> {
    let published = [
        work.issued.as_ref(),
        work.published_online.as_ref(),
        work.published_print.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(bound)
    .reduce(|best, next| {
        if merge::view::earlier(&next, &best).is_lt() {
            next
        } else {
            best
        }
    });
    let mut issns = smallvec::SmallVec::new();
    for t in &work.issns {
        if let Claim::Valid(issn) = &t.issn
            && !issns.contains(issn)
        {
            issns.push(*issn);
        }
    }
    let mut open = None;
    for license in &work.licenses {
        if text(&license.url, table, "licenses.url")?.contains("creativecommons.org") {
            open = Some(true);
        }
    }
    let by = |kinds: &[UpdateType]| work.updated_by.iter().any(|u| kinds.contains(&u.kind));
    let genre = genre(work);
    Ok(View {
        version: 1,
        set: "",
        revised: Some(work.indexed.date()),
        titles: <crate::Crossref as merge::Contributor>::titles(work),
        has_abstract: work.r#abstract.is_some(),
        authors: u32::try_from(work.authors.len()).unwrap_or(u32::MAX),
        venue: Venue {
            openalex: None,
            issns,
            nlm: None,
        },
        published,
        language: work.language,
        form: Some(form(work.kind)),
        genre: Some(genre),
        status: Status {
            retracted: by(&[
                UpdateType::Retraction,
                UpdateType::PartialRetraction,
                UpdateType::Withdrawal,
                UpdateType::Removal,
            ]),
            corrected: by(&[
                UpdateType::Correction,
                UpdateType::Corrigendum,
                UpdateType::Erratum,
            ]),
            concern: by(&[UpdateType::ExpressionOfConcern]),
        },
        access: open,
        references: work.references_count,
        cited_by: work.cited_by_count,
    })
}

/// A relation target as a name, when its id is in our namespaces and
/// parses.
fn target(id: &str, kind: RelationIdKind) -> Option<OwnedName> {
    match kind {
        RelationIdKind::Doi => Doi::parse(id)
            .ok()
            .map(|d| OwnedName::Doi(d.as_str().into())),
        RelationIdKind::Pmid => Pmid::from_digits(id).ok().map(OwnedName::Pmid),
        RelationIdKind::Pmcid => PmcId::from_prefixed(id)
            .or_else(|_| PmcId::from_digits(id))
            .ok()
            .map(OwnedName::Pmc),
        _ => None,
    }
}

/// The record's relations: `relation` entries by type, then Crossmark
/// updates both ways.
///
/// # Errors
///
/// A relation type outside the table, or a symbol beyond the set's
/// table.
pub fn relations(
    work: &Work<'_>,
    table: &Table,
    out: &mut dyn FnMut(Related),
) -> Result<(), Unmapped> {
    for r in &work.relations {
        let term = text(&r.kind, table, "relations.kind")?;
        let kind = relation_kind(term).ok_or_else(|| Unmapped {
            field: "relations.kind",
            value: term.to_owned(),
        })?;
        out(Related {
            kind,
            target: target(r.id, r.id_kind),
        });
    }
    for u in &work.updates_to {
        out(Related {
            kind: Kind::UpdateTo(update(u.kind)),
            target: Some(OwnedName::Doi(u.doi.as_str().into())),
        });
    }
    for u in &work.updated_by {
        out(Related {
            kind: Kind::UpdatedBy(update(u.kind)),
            target: Some(OwnedName::Doi(u.doi.as_str().into())),
        });
    }
    Ok(())
}

/// What a reference gives besides its DOI.
#[must_use]
pub fn citation<'w>(reference: &'w Reference<'_>) -> Citation<'w> {
    Citation {
        text: reference.unstructured,
        key: Some(reference.key),
        year: reference.year,
        author: reference.author,
        title: reference.article_title,
        container: reference
            .journal_title
            .or(reference.volume_title)
            .or(reference.series_title)
            .map(Interned::Text),
        volume: reference.volume,
        issue: reference.issue,
        first_page: reference.first_page,
    }
}

/// The name a reference carries, for tests of the target rule.
#[must_use]
pub fn reference_name<'w>(reference: &'w Reference<'_>) -> Option<Name<'w>> {
    reference
        .doi
        .as_ref()
        .and_then(Claim::valid)
        .map(|d| Name::Doi(d.as_str()))
}

/// The record's text for the serving layer: the JATS abstract as plain
/// text, `given family` or the organisation per author, the container
/// title.
///
/// # Errors
///
/// A symbol beyond the set's table.
pub fn text_of(work: &Work<'_>, table: &Table) -> Result<Text, Unmapped> {
    let abstract_text = work.r#abstract.map(strip_markup).filter(|t| !t.is_empty());
    let authors = work
        .authors
        .iter()
        .map(|a| match a.name {
            AuthorName::Person { given, family } => match given {
                Some(given) => format!("{given} {family}"),
                None => family.to_owned(),
            },
            AuthorName::Org(name) => name.to_owned(),
        })
        .collect();
    let venue = match &work.container_title {
        Some(title) => Some(text(title, table, "container_title")?.to_owned()),
        None => None,
    };
    Ok(Text {
        abstract_text,
        authors,
        venue: venue.filter(|v| !v.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use merge::kind::{Form, Genre};
    use merge::relation::Kind;

    use super::{form, relation_kind};
    use crate::vocab::WorkType;

    #[test]
    fn every_work_type_has_a_form_and_the_sample_relation_types_map() {
        assert_eq!(form(WorkType::JournalArticle), Form::JournalArticle);
        assert_eq!(form(WorkType::PostedContent), Form::PostedContent);
        assert!(Form::PostedContent.admits(Genre::Preprint));
        for word in [
            "has-review",
            "has-preprint",
            "is-supplemented-by",
            "is-referenced-by",
            "references",
            "is-version-of",
            "correction",
            "is-same-as",
            "erratum",
            "is-component-of",
            "has-version",
            "is-translation-of",
            "documents",
            "new_version",
            "is-part-of",
            "is-cited-by",
            "is-preprint-of",
            "has-part",
            "is-review-of",
            "corrigendum",
            "is-basis-for",
            "is-financed-by",
            "has-derivation",
            "has-manifestation",
            "is-manuscript-of",
            "is-comment-on",
            "is-identical-to",
            "expression_of_concern",
        ] {
            assert!(relation_kind(word).is_some(), "{word}");
        }
        assert_eq!(relation_kind("new_version"), Some(Kind::NewVersion));
        assert_eq!(relation_kind("is-friend-of"), None);
    }
}
