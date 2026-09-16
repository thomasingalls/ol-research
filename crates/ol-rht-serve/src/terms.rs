//! The terms module, as a stub (serving plan §7): one attribution line
//! per source, and whether a source's abstract may go on the wire — a
//! table to edit when the terms pass happens, every switch on for the
//! proof of concept.

use std::collections::BTreeMap;

use merge::{SourceId, Sources};

/// What each source asks of an answer that draws on it.
#[derive(Debug, Clone)]
pub struct Terms {
    attribution: BTreeMap<SourceId, String>,
    abstracts: BTreeMap<SourceId, bool>,
}

impl Terms {
    /// The stub: attribution lines as noted 2026-09-15, abstracts allowed
    /// from every source until the terms pass says otherwise.
    #[must_use]
    pub fn stub() -> Self {
        let attribution = [
            (
                SourceId::Pubmed,
                "PubMed data courtesy of the U.S. National Library of Medicine, which does not endorse or recommend any product or service; current as of the epoch's date.",
            ),
            (SourceId::Crossref, "Metadata from Crossref."),
            (SourceId::OpenAlex, "Data from OpenAlex, CC0."),
        ]
        .into_iter()
        .map(|(s, line)| (s, line.to_owned()))
        .collect();
        let abstracts = SourceId::ALL.into_iter().map(|s| (s, true)).collect();
        Self {
            attribution,
            abstracts,
        }
    }

    /// The attribution lines for an answer drawing on `sources`, in
    /// source order.
    #[must_use]
    pub fn attribution(&self, sources: Sources) -> Vec<String> {
        sources
            .iter()
            .filter_map(|s| self.attribution.get(&s).cloned())
            .collect()
    }

    /// Whether a source's abstract may go on the wire.
    #[must_use]
    pub fn abstract_allowed(&self, source: SourceId) -> bool {
        self.abstracts.get(&source).copied().unwrap_or(false)
    }

    /// Every source.
    #[must_use]
    pub fn all() -> Sources {
        SourceId::ALL.into_iter().collect()
    }
}
