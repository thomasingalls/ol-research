//! Typed relations between works (merge plan §6): everything a source
//! says about another work that is not a citation and not an identity
//! assertion — errata, retractions, comments, preprints, supplements,
//! versions — as a closed vocabulary that unions the three sources',
//! each source's word kept, the source recorded on every row.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::name::OwnedName;

/// A Crossmark update type, as Crossref spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum Update {
    Addendum,
    Clarification,
    Correction,
    Corrigendum,
    Erratum,
    ExpressionOfConcern,
    NewEdition,
    NewVersion,
    PartialRetraction,
    Removal,
    Retraction,
    Withdrawal,
}

impl Update {
    /// The spelling in reports and sets.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Addendum => "addendum",
            Self::Clarification => "clarification",
            Self::Correction => "correction",
            Self::Corrigendum => "corrigendum",
            Self::Erratum => "erratum",
            Self::ExpressionOfConcern => "expression-of-concern",
            Self::NewEdition => "new-edition",
            Self::NewVersion => "new-version",
            Self::PartialRetraction => "partial-retraction",
            Self::Removal => "removal",
            Self::Retraction => "retraction",
            Self::Withdrawal => "withdrawal",
        }
    }
}

/// The kind of a relation, from the asserting record's side: PubMed's
/// `CommentsCorrections` types, Crossref's `relation` types and the
/// Crossmark words some publishers deposit among them, Crossmark updates
/// in both directions, and `has-version` for an OpenAlex location whose
/// DOI is another work's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum Kind {
    // PubMed `RefType`.
    AssociatedDataset,
    AssociatedPublication,
    CommentIn,
    CommentOn,
    CorrectedAndRepublishedIn,
    CorrectedAndRepublishedFrom,
    ErratumIn,
    ErratumFor,
    ExpressionOfConcernIn,
    ExpressionOfConcernFor,
    RepublishedIn,
    RepublishedFrom,
    RetractedAndRepublishedIn,
    RetractedAndRepublishedFrom,
    RetractionIn,
    RetractionOf,
    UpdateIn,
    UpdateOf,
    SummaryForPatientsIn,
    OriginalReportIn,
    ReprintIn,
    ReprintOf,
    Cites,
    // Crossref `relation` types (the deposit schema's inter-work
    // relations, plus `is-cited-by` and `is-component-of` seen in data).
    HasReview,
    IsReviewOf,
    HasPreprint,
    IsPreprintOf,
    IsSupplementedBy,
    IsSupplementTo,
    IsReferencedBy,
    References,
    IsCitedBy,
    IsVersionOf,
    HasVersion,
    IsSameAs,
    IsIdenticalTo,
    IsComponentOf,
    HasPart,
    IsPartOf,
    IsTranslationOf,
    HasTranslation,
    Documents,
    IsDocumentedBy,
    IsBasisFor,
    IsBasedOn,
    IsFinancedBy,
    Finances,
    HasDerivation,
    IsDerivedFrom,
    HasManifestation,
    IsManifestationOf,
    IsManuscriptOf,
    HasManuscript,
    IsCommentOn,
    HasComment,
    IsReplyTo,
    HasReply,
    IsOriginalFormOf,
    IsVariantFormOf,
    IsExpressionOf,
    HasExpression,
    IsFormatOf,
    HasFormat,
    IsRelatedMaterial,
    HasRelatedMaterial,
    Requires,
    IsRequiredBy,
    IsContinuedBy,
    Continues,
    Compiles,
    IsCompiledBy,
    IsReplacedBy,
    Replaces,
    IsDataBasisFor,
    BasedOnData,
    // Crossmark words deposited as `relation` types.
    Correction,
    Erratum,
    Corrigendum,
    NewVersion,
    ExpressionOfConcern,
    Addendum,
    Clarification,
    NewEdition,
    PartialRetraction,
    Removal,
    Retraction,
    Withdrawal,
    // Crossmark updates: this record is the notice that updates the
    // target, or the target is the notice that updates this record.
    UpdateTo(Update),
    UpdatedBy(Update),
}

impl Kind {
    /// The spelling in reports: the set's spelling for a unit kind, and
    /// `update-to:<type>` / `updated-by:<type>` for an update.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AssociatedDataset => "associated-dataset",
            Self::AssociatedPublication => "associated-publication",
            Self::CommentIn => "comment-in",
            Self::CommentOn => "comment-on",
            Self::CorrectedAndRepublishedIn => "corrected-and-republished-in",
            Self::CorrectedAndRepublishedFrom => "corrected-and-republished-from",
            Self::ErratumIn => "erratum-in",
            Self::ErratumFor => "erratum-for",
            Self::ExpressionOfConcernIn => "expression-of-concern-in",
            Self::ExpressionOfConcernFor => "expression-of-concern-for",
            Self::RepublishedIn => "republished-in",
            Self::RepublishedFrom => "republished-from",
            Self::RetractedAndRepublishedIn => "retracted-and-republished-in",
            Self::RetractedAndRepublishedFrom => "retracted-and-republished-from",
            Self::RetractionIn => "retraction-in",
            Self::RetractionOf => "retraction-of",
            Self::UpdateIn => "update-in",
            Self::UpdateOf => "update-of",
            Self::SummaryForPatientsIn => "summary-for-patients-in",
            Self::OriginalReportIn => "original-report-in",
            Self::ReprintIn => "reprint-in",
            Self::ReprintOf => "reprint-of",
            Self::Cites => "cites",
            Self::HasReview => "has-review",
            Self::IsReviewOf => "is-review-of",
            Self::HasPreprint => "has-preprint",
            Self::IsPreprintOf => "is-preprint-of",
            Self::IsSupplementedBy => "is-supplemented-by",
            Self::IsSupplementTo => "is-supplement-to",
            Self::IsReferencedBy => "is-referenced-by",
            Self::References => "references",
            Self::IsCitedBy => "is-cited-by",
            Self::IsVersionOf => "is-version-of",
            Self::HasVersion => "has-version",
            Self::IsSameAs => "is-same-as",
            Self::IsIdenticalTo => "is-identical-to",
            Self::IsComponentOf => "is-component-of",
            Self::HasPart => "has-part",
            Self::IsPartOf => "is-part-of",
            Self::IsTranslationOf => "is-translation-of",
            Self::HasTranslation => "has-translation",
            Self::Documents => "documents",
            Self::IsDocumentedBy => "is-documented-by",
            Self::IsBasisFor => "is-basis-for",
            Self::IsBasedOn => "is-based-on",
            Self::IsFinancedBy => "is-financed-by",
            Self::Finances => "finances",
            Self::HasDerivation => "has-derivation",
            Self::IsDerivedFrom => "is-derived-from",
            Self::HasManifestation => "has-manifestation",
            Self::IsManifestationOf => "is-manifestation-of",
            Self::IsManuscriptOf => "is-manuscript-of",
            Self::HasManuscript => "has-manuscript",
            Self::IsCommentOn => "is-comment-on",
            Self::HasComment => "has-comment",
            Self::IsReplyTo => "is-reply-to",
            Self::HasReply => "has-reply",
            Self::IsOriginalFormOf => "is-original-form-of",
            Self::IsVariantFormOf => "is-variant-form-of",
            Self::IsExpressionOf => "is-expression-of",
            Self::HasExpression => "has-expression",
            Self::IsFormatOf => "is-format-of",
            Self::HasFormat => "has-format",
            Self::IsRelatedMaterial => "is-related-material",
            Self::HasRelatedMaterial => "has-related-material",
            Self::Requires => "requires",
            Self::IsRequiredBy => "is-required-by",
            Self::IsContinuedBy => "is-continued-by",
            Self::Continues => "continues",
            Self::Compiles => "compiles",
            Self::IsCompiledBy => "is-compiled-by",
            Self::IsReplacedBy => "is-replaced-by",
            Self::Replaces => "replaces",
            Self::IsDataBasisFor => "is-data-basis-for",
            Self::BasedOnData => "based-on-data",
            Self::Correction => "correction",
            Self::Erratum => "erratum",
            Self::Corrigendum => "corrigendum",
            Self::NewVersion => "new-version",
            Self::ExpressionOfConcern => "expression-of-concern",
            Self::Addendum => "addendum",
            Self::Clarification => "clarification",
            Self::NewEdition => "new-edition",
            Self::PartialRetraction => "partial-retraction",
            Self::Removal => "removal",
            Self::Retraction => "retraction",
            Self::Withdrawal => "withdrawal",
            Self::UpdateTo(_) => "update-to",
            Self::UpdatedBy(_) => "updated-by",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpdateTo(u) | Self::UpdatedBy(u) => write!(f, "{}:{}", self.name(), u.name()),
            other => f.write_str(other.name()),
        }
    }
}

/// One relation a record asserts: its kind and the other work's name, or
/// `None` for a target outside our namespaces (a URI, an accession
/// number, an ISSN), which is counted and not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Related {
    #[allow(missing_docs)]
    pub kind: Kind,
    #[allow(missing_docs)]
    pub target: Option<OwnedName>,
}

#[cfg(test)]
mod tests {
    use super::{Kind, Update};

    #[test]
    fn kinds_spell_themselves() {
        assert_eq!(
            serde_json::to_string(&Kind::CorrectedAndRepublishedIn).unwrap(),
            "\"corrected-and-republished-in\""
        );
        assert_eq!(
            serde_json::to_string(&Kind::UpdateTo(Update::ExpressionOfConcern)).unwrap(),
            "{\"update-to\":\"expression-of-concern\"}"
        );
        assert_eq!(Kind::HasPreprint.to_string(), "has-preprint");
        assert_eq!(
            Kind::UpdatedBy(Update::Retraction).to_string(),
            "updated-by:retraction"
        );
        assert_eq!(
            serde_json::from_str::<Kind>("\"is-supplemented-by\"").unwrap(),
            Kind::IsSupplementedBy
        );
        assert!(Kind::CommentIn < Kind::UpdateTo(Update::Addendum));
    }
}
