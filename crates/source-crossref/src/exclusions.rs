//! Records kept out of the normalized layer on purpose (ingestion plan §6,
//! the fix loop's third way): each is one deposit of a Crossref record —
//! its DOI and its `deposited` stamp — listed with why. The run normalizes
//! everything else, and the stats report counts what the list kept out. A
//! later deposit of a listed DOI is not on the list: it normalizes, or
//! stops the run, on its own.
//!
//! Today the list is test deposits, which are not works: F1000's
//! `f1000devtestarticle…` of 2012 and a record on Crossref's own test
//! prefix, `10.5555`, each carrying an update word no publisher meant
//! (`sample_update`, `this_is_some_update_23`, `interesting_update`) —
//! the three among the records the March 2026 file's survey stopped at
//! (2026-09-26, homelab `ol-rht-dump-crossref-v1`; the user's call), and
//! the three more the survey of the whole file then stopped at as a test
//! word on a record not listed (`ol-rht-survey-crossref-v2`), their lines
//! found by DOI (`ol-rht-dump-crossref-v2`), and one more behind them. A
//! test deposit not listed stops a run the same way, and joins the list
//! from the same evidence.

use model::{DateTime, Doi};

struct Excluded {
    doi: &'static str,
    deposited: &'static str,
    reason: &'static str,
}

const TEST: &str = "a test deposit";

const EXCLUDED: [Excluded; 7] = [
    // Shard 25809, line 4647: updated by b and c, `sample_update`.
    Excluded {
        doi: "10.3410/f1000devtestarticlea",
        deposited: "2012-05-31T08:39:41Z",
        reason: TEST,
    },
    // Shard 25810, line 4698.
    Excluded {
        doi: "10.3410/f1000devtestarticleb",
        deposited: "2012-05-31T08:42:21Z",
        reason: TEST,
    },
    // Shard 25811, line 4698.
    Excluded {
        doi: "10.3410/f1000devtestarticlec",
        deposited: "2012-05-31T08:45:05Z",
        reason: TEST,
    },
    // Shard 25797, line 3373.
    Excluded {
        doi: "10.3410/f1000devtestarticled",
        deposited: "2012-06-06T08:54:10Z",
        reason: TEST,
    },
    // Shard 26156, line 4923: "Society of Psychoceramics".
    Excluded {
        doi: "10.5555/26262626x",
        deposited: "2019-07-25T14:55:16Z",
        reason: TEST,
    },
    // Shard 26154, line 4935: updated by 26262626x, `interesting_update`.
    Excluded {
        doi: "10.5555/12345680",
        deposited: "2025-11-14T10:59:00Z",
        reason: TEST,
    },
    // Shard 10373, line 4764: a dissertation by "Test accounts" whose
    // affiliation carries an ISNI that fails its check character and a
    // Wikidata property, `P5188229`, where an item goes.
    Excluded {
        doi: "10.5555/diswikitest",
        deposited: "2022-01-18T19:50:55Z",
        reason: TEST,
    },
];

/// Why the list keeps this deposit out, if it does.
#[must_use]
pub fn reason(doi: &Doi<'_>, deposited: DateTime) -> Option<&'static str> {
    EXCLUDED
        .iter()
        .find(|e| e.doi == doi.as_str() && DateTime::parse_utc(e.deposited).ok() == Some(deposited))
        .map(|e| e.reason)
}

#[cfg(test)]
mod tests {
    use super::{EXCLUDED, reason};
    use model::{DateTime, Doi};

    #[test]
    fn a_deposit_is_kept_out_and_a_later_one_is_not() {
        let doi = Doi::parse("10.3410/F1000DEVTESTARTICLEC").unwrap();
        let listed = DateTime::parse_utc("2012-05-31T08:45:05Z").unwrap();
        assert_eq!(reason(&doi, listed), Some("a test deposit"));
        let later = DateTime::parse_utc("2026-01-01T00:00:00Z").unwrap();
        assert_eq!(reason(&doi, later), None);
    }

    #[test]
    fn every_entry_is_well_formed() {
        for e in &EXCLUDED {
            let doi = Doi::parse(e.doi).unwrap();
            assert_eq!(doi.as_str(), e.doi, "written as the normalized DOI");
            assert!(DateTime::parse_utc(e.deposited).is_ok(), "{}", e.deposited);
        }
    }
}
