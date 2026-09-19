//! Records kept out of the normalized layer on purpose (ingestion plan §6,
//! the fix loop's third way): each is one revision of a PubMed record —
//! its PMID, its version and its `DateRevised` — that holds something the
//! model cannot say yet, listed with what that is. The run normalizes
//! everything else, and the stats report counts what the list kept out. A
//! later revision of a listed record is not on the list: it normalizes, or
//! stops the run, on its own.
//!
//! Each entry waits on a decision or a model change, not on a parser.
//!
//! The list is empty since 2026-09-18: the seven records it held are
//! normalized again, the four shapes they showed having become model
//! changes on the user's word — a record may give two DOIs, more than one
//! publisher item id, a `pmc` slot holding something else (kept as a
//! malformed claim, never a name), and a reference may carry an id of any
//! other kind (kept, never a name). The mechanism stays for the next
//! shape that needs a decision before it can be read.

use model::{Date, Pmid};

struct Excluded {
    pmid: u32,
    version: u32,
    revised: &'static str,
    reason: &'static str,
}

/// Empty: nothing is kept out today (2026-09-18).
const EXCLUDED: [Excluded; 0] = [];

/// Why the list keeps a record's revision out, if it does.
#[must_use]
pub fn reason(pmid: Pmid, version: u32, revised: Option<Date>) -> Option<&'static str> {
    let revised = revised?.to_string();
    EXCLUDED
        .iter()
        .find(|e| e.pmid == pmid.get() && e.version == version && e.revised == revised)
        .map(|e| e.reason)
}

#[cfg(test)]
mod tests {
    use super::{EXCLUDED, reason};
    use model::{Date, Pmid};

    #[test]
    fn an_empty_list_keeps_nothing_out() {
        let pmid = Pmid::new(19_257_808).unwrap();
        assert_eq!(reason(pmid, 1, Date::parse_iso("2026-01-28").ok()), None);
        assert_eq!(reason(pmid, 1, None), None);
    }

    #[test]
    fn every_entry_is_well_formed() {
        for e in &EXCLUDED {
            assert!(Pmid::new(e.pmid).is_some(), "{}", e.pmid);
            assert!(Date::parse_iso(e.revised).is_ok(), "{}", e.revised);
        }
    }
}
