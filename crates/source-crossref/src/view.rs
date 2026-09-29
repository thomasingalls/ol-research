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
use crate::vocab::{Meaning, RelationAssertedBy, RelationIdKind, UpdateType, UpdateWord, WorkType};

/// What scopes the record's publisher ids (merge plan §3.1): its DOI,
/// which every Crossref record has by construction. Not `prefix`, which
/// names the DOI's current owner and differs from its registrant on 7 %
/// of works — a publisher id was issued by whoever registered the DOI,
/// not by whoever holds it now.
#[must_use]
pub fn publisher_scope<'w>(work: &'w Work<'_>) -> merge::publisher::Scope<'w> {
    let mut scope = merge::publisher::Scope::new();
    scope.doi(&work.doi);
    scope
}

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

/// The genre a Crossmark notice asserts — a record whose `update-to` is a
/// retraction is the retraction notice, one that comments on a work is a
/// comment — or else the one its form implies when nothing more is known,
/// the third choice of §5's precedence. `None` for a record with neither:
/// no notice, and no type.
///
/// An update that names the record itself makes it no notice: it is the
/// work, updated in place — APS's `corrected` articles (35 among the
/// records the March 2026 file's survey stopped at), and a few
/// `correction`, `retraction` and `new_edition` updates written the same
/// way. It still counts in [`Status`], from `updated-by`.
#[must_use]
pub fn genre(work: &Work<'_>) -> Option<Genre> {
    let notices = || work.updates_to.iter().filter(|u| u.doi != work.doi);
    // The twelve Crossmark types by their order, as before a comment
    // joined the list.
    let notice = notices()
        .map(|u| u.kind)
        .filter(|k| *k != UpdateType::Comment)
        .max();
    match notice {
        Some(UpdateType::Retraction | UpdateType::PartialRetraction | UpdateType::Withdrawal) => {
            return Some(Genre::Retraction);
        }
        Some(UpdateType::Correction | UpdateType::Corrigendum | UpdateType::Erratum) => {
            return Some(Genre::Erratum);
        }
        Some(UpdateType::ExpressionOfConcern) => return Some(Genre::ExpressionOfConcern),
        _ => {}
    }
    if notices().any(|u| u.kind == UpdateType::Comment) {
        return Some(Genre::Comment);
    }
    work.kind.map(default_genre)
}

/// The genre a work type implies when nothing more is known.
#[must_use]
pub const fn default_genre(kind: WorkType) -> Genre {
    match kind {
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

/// An update as the merge's relation, from the notice's side
/// (`update-to`) or the updated work's (`updated-by`). The merge's
/// updates are Crossmark's twelve; a comment is its comment relation
/// instead.
#[must_use]
pub const fn update_relation(kind: UpdateType, notice: bool) -> Kind {
    let update = match kind {
        UpdateType::Comment if notice => return Kind::IsCommentOn,
        UpdateType::Comment => return Kind::HasComment,
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
    };
    if notice {
        Kind::UpdateTo(update)
    } else {
        Kind::UpdatedBy(update)
    }
}

/// A `relation` type as the merge's relation kind: the deposit schema's
/// inter-work relations, three more the data showed (`is-cited-by`,
/// `is-component-of`, and `cites`, PubMed's word too, on 165 records of
/// the March 2026 file), and update words.
/// Crossref copies a work's `updated-by` into its `relation` under the
/// update's word, asserted by the object, the notice: those read as what
/// the word means ([`UpdateWord::meaning`]), and a comment so asserted is
/// a comment on this work. A word that is no update never gets here: the
/// normalizer set it aside.
#[must_use]
pub fn relation_kind(kind: &str, by: RelationAssertedBy) -> Option<Kind> {
    if let Some(word) = UpdateWord::from_word(kind) {
        let Meaning::Update(update) = word.meaning() else {
            return None;
        };
        return Some(match update {
            UpdateType::Comment if by == RelationAssertedBy::Object => Kind::HasComment,
            UpdateType::Comment => return None,
            UpdateType::Correction => Kind::Correction,
            UpdateType::Erratum => Kind::Erratum,
            UpdateType::Corrigendum => Kind::Corrigendum,
            UpdateType::NewVersion => Kind::NewVersion,
            UpdateType::ExpressionOfConcern => Kind::ExpressionOfConcern,
            UpdateType::Addendum => Kind::Addendum,
            UpdateType::Clarification => Kind::Clarification,
            UpdateType::NewEdition => Kind::NewEdition,
            UpdateType::PartialRetraction => Kind::PartialRetraction,
            UpdateType::Removal => Kind::Removal,
            UpdateType::Retraction => Kind::Retraction,
            UpdateType::Withdrawal => Kind::Withdrawal,
        });
    }
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
        "cites" => Kind::Cites,
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
    Ok(View {
        version: 1,
        set: "",
        superseded: Vec::new(),
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
        form: work.kind.map(form),
        genre: genre(work),
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
            // Crossmark has no update type for it: a duplicate is
            // PubMed's word, and only PubMed's.
            duplicate: false,
        },
        access: open,
        references: work.references_count,
        cited_by: work.cited_by_count,
    })
}

/// A relation target as a name, when its id is in our namespaces and
/// parses.
/// `short-doi` is read as a DOI: every one the March 2026 file's survey
/// reached is a whole DOI, and one that is not stays no name.
fn target(id: &str, kind: RelationIdKind) -> Option<OwnedName> {
    match kind {
        RelationIdKind::Doi | RelationIdKind::ShortDoi => Doi::parse(id)
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
        let kind = relation_kind(term, r.asserted_by).ok_or_else(|| Unmapped {
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
            kind: update_relation(u.kind, true),
            target: Some(OwnedName::Doi(u.doi.as_str().into())),
        });
    }
    for u in &work.updated_by {
        out(Related {
            kind: update_relation(u.kind, false),
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
            AuthorName::Org(name) | AuthorName::Given(name) => name.to_owned(),
        })
        .collect();
    // The first title, as model 12 kept it: which of the rest names what
    // is the chain's to read, by type (citation fields plan §4).
    let venue = match work.container_titles.first() {
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

    use super::{form, relation_kind, update_relation};
    use crate::vocab::{RelationAssertedBy, UpdateType, WorkType};

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
            assert!(
                relation_kind(word, RelationAssertedBy::Subject).is_some(),
                "{word}"
            );
        }
        let object = |word| relation_kind(word, RelationAssertedBy::Object);
        assert_eq!(object("new_version"), Some(Kind::NewVersion));
        assert_eq!(object("is-friend-of"), None);
        // The old words, as the dump showed them under `relation`: a copy
        // of the work's `updated-by`, asserted by the notice.
        assert_eq!(object("err"), Some(Kind::Erratum));
        assert_eq!(object("corrected-article"), Some(Kind::Correction));
        assert_eq!(object("publisher-note"), Some(Kind::Correction));
        assert_eq!(
            object("expression-of-concern"),
            Some(Kind::ExpressionOfConcern)
        );
        assert_eq!(object("comment"), Some(Kind::HasComment));
        assert_eq!(object("correspondence"), Some(Kind::HasComment));
        assert_eq!(
            relation_kind("comment", RelationAssertedBy::Subject),
            None,
            "which way it points is not said"
        );
        assert_eq!(object("article"), None, "set aside, never here");
    }

    /// Every `relation` type the March 2026 file writes, as the normalize
    /// of the whole file counted them (homelab
    /// `ol-rht-normalize-crossref-v1`, 2026-09-26), maps: a type that did
    /// not would stop a merge of record. `cites`, on 165 records, was the
    /// one the merge survey over an eighth of the file found missing
    /// (`ol-rht-merge-survey-v6`). The update words among them are read
    /// as asserted by the notice, as the file's copies of `updated-by`
    /// are; a comment the work itself asserts still maps to nothing.
    #[test]
    fn every_relation_type_of_the_march_2026_file_maps() {
        assert_eq!(
            relation_kind("cites", RelationAssertedBy::Subject),
            Some(Kind::Cites)
        );
        for word in [
            "addendum",
            "based-on-data",
            "cites",
            "clarification",
            "comment",
            "compiles",
            "continues",
            "corrected",
            "corrected-article",
            "correction",
            "correspondence",
            "corrigendum",
            "documents",
            "err",
            "erratum",
            "expression-of-concern",
            "expression_of_concern",
            "finances",
            "has-comment",
            "has-derivation",
            "has-expression",
            "has-format",
            "has-manifestation",
            "has-manuscript",
            "has-part",
            "has-preprint",
            "has-related-material",
            "has-reply",
            "has-review",
            "has-translation",
            "has-version",
            "is-based-on",
            "is-basis-for",
            "is-cited-by",
            "is-comment-on",
            "is-compiled-by",
            "is-component-of",
            "is-continued-by",
            "is-data-basis-for",
            "is-derived-from",
            "is-documented-by",
            "is-expression-of",
            "is-financed-by",
            "is-format-of",
            "is-identical-to",
            "is-manifestation-of",
            "is-manuscript-of",
            "is-original-form-of",
            "is-part-of",
            "is-preprint-of",
            "is-referenced-by",
            "is-related-material",
            "is-replaced-by",
            "is-reply-to",
            "is-required-by",
            "is-review-of",
            "is-same-as",
            "is-supplement-to",
            "is-supplemented-by",
            "is-translation-of",
            "is-variant-form-of",
            "is-version-of",
            "new_edition",
            "new_version",
            "publisher-note",
            "references",
            "removal",
            "replaces",
            "requires",
            "retraction",
            "withdrawal",
        ] {
            assert!(
                relation_kind(word, RelationAssertedBy::Object).is_some(),
                "{word}"
            );
        }
    }

    #[test]
    fn a_comment_is_the_merge_relation_it_is() {
        assert_eq!(
            update_relation(UpdateType::Comment, true),
            Kind::IsCommentOn
        );
        assert_eq!(
            update_relation(UpdateType::Comment, false),
            Kind::HasComment
        );
        assert_eq!(
            update_relation(UpdateType::Correction, false),
            Kind::UpdatedBy(merge::relation::Update::Correction)
        );
        assert!(Form::JournalArticle.admits(Genre::Comment));
    }
}
