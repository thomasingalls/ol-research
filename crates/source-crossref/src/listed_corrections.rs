//! Deposits whose aliases are the works they correct, though nothing the
//! merge reads says so: the merge's answer to
//! [`merge::Contributor::corrects_its_aliases`] (merge plan §11 item 28).
//! Each is one deposit of a Crossref record, its DOI and its `deposited`
//! stamp, listed with why. The merge then refuses its aliases on its
//! record as names its own work corrects, as it would have had a record of
//! the work said so. A later deposit of a listed DOI is not on the list:
//! the merge judges it on its own, or stops on it.
//!
//! This is a compromise, and the list should go when freshness takes in
//! PubMed's update files (merge plan §11 item 28, 2026-09-28). Today it
//! holds one correction, `10.1161/svin.122.000762`, whose deposit gives
//! the 102 supplement abstracts it corrects as its aliases, and so joins
//! them into one work. PubMed's current copy of its notice, PMID
//! 41585761, is the erratum for all 102, which settles it under the
//! merge's rules (model version 12). But the 2026 baseline's copy
//! predates those links, and the survey of the whole file at model 12
//! stopped on this deposit alone (homelab `ol-rht-merge-survey-v9`). The
//! user's call, 2026-09-27: a list for now; the update files when
//! freshness brings them in, and then the list goes.

use model::{DateTime, Doi};

struct Listed {
    doi: &'static str,
    deposited: &'static str,
}

const LISTED: [Listed; 1] = [
    // Stroke: Vascular and Interventional Neurology, "Correction to:
    // Stroke: Vascular and Interventional Neurology Volume 1, No. S1".
    Listed {
        doi: "10.1161/svin.122.000762",
        deposited: "2024-05-13T11:40:20Z",
    },
];

/// Whether this deposit's aliases are what it corrects.
#[must_use]
pub fn corrects_its_aliases(doi: &Doi<'_>, deposited: DateTime) -> bool {
    LISTED
        .iter()
        .any(|l| l.doi == doi.as_str() && DateTime::parse_utc(l.deposited).ok() == Some(deposited))
}

#[cfg(test)]
mod tests {
    use super::{LISTED, corrects_its_aliases};
    use model::{DateTime, Doi};

    #[test]
    fn a_deposit_is_listed_and_a_later_one_is_not() {
        let doi = Doi::parse("10.1161/SVIN.122.000762").unwrap();
        let listed = DateTime::parse_utc("2024-05-13T11:40:20Z").unwrap();
        assert!(corrects_its_aliases(&doi, listed));
        let later = DateTime::parse_utc("2026-09-27T00:00:00Z").unwrap();
        assert!(!corrects_its_aliases(&doi, later));
    }

    #[test]
    fn every_listed_deposit_is_a_doi_and_a_stamp() {
        for l in &LISTED {
            let doi = Doi::parse(l.doi).unwrap();
            assert_eq!(doi.as_str(), l.doi, "listed as the DOI of record");
            DateTime::parse_utc(l.deposited).unwrap();
        }
    }
}
