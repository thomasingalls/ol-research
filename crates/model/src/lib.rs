//! Canonical types for the normalized layer.
//!
//! Only what is committed lives here (plan §4): each source's native
//! identifier, with the exact rule it enforces. Everything else about the
//! normalized shape is discovered from data, not declared ahead of it.

/// The model version. Bump it on any change to a type here or to a
/// source's normalized types: normalized output carries it (plan §6), and
/// output written under another version is invalid — rebuilt, never
/// patched.
///
/// Version 2, 2026-09-18: a PubMed record keeps every publisher item id it
/// gives rather than one, its `pmc` slot is a claim so a value that is not
/// a PMC id is kept as written, and a reference keeps every other id kind
/// it carries. Ids the model cannot place are kept and never matched on
/// (ingestion plan §6).
///
/// Version 3, 2026-09-23: a publication date can be a seasonal volume
/// (`PartialDate::Seasons`) — the years a volume spans with the season or
/// seasons its issue names, as PubMed's oldest baseline files write them
/// (`1976-1977 Winter`, `1977-1978 Fall-Winter`). Modelled rather than
/// flattened to a year span, because the old records are full of them
/// (the user's call, 2026-09-23).
///
/// Version 4, 2026-09-23: a PubMed record can carry no publication types
/// at all. `PublicationTypeList` is declared `(PublicationType+)`, and
/// 26 records of the 2026 baseline, ~37M records, write it with nothing
/// inside it — the element present every time, never absent. Five of the
/// 26 are `Status="MEDLINE"` with MeSH headings, so an empty list is a
/// shape NLM ships in fully indexed records and not a gap curation will
/// close; the parser now reads it as written instead of rejecting the
/// record. Nothing downstream changes: a work with no genre was already
/// what the merge's `Option<Genre>` means, and these records simply
/// never reached it. Normalized output under version 3 is rebuilt, not
/// patched, because it is missing those records.
/// Version 5 (2026-09-23): the last of what the survey of the whole 2026
/// baseline found — 11,343 records in 39,983,661 that this build could
/// not read. Three shapes the model had never been taught: a
/// `PubMedPubDate` may stop at the hour, so [`date::TimeOfDay`] holds an
/// optional minute rather than inventing a `:00` no later reader could
/// tell from a stated zero; an `ELocationID` marked `ValidYN="N"` is NLM
/// saying an identifier is the wrong one, so it is kept in its own slot,
/// where it can still match a citation that repeats the mistake, and is
/// never a name; and a record's own id list may carry the kinds that name
/// a deposit or a Bookshelf document rather than a work, which join the
/// loose ids of version 2. Three more were PubMed disagreeing with
/// itself, and needed no new shape: two assertions of one identifier
/// agree or they say nothing, a `PMCnnn.v` suffix is PMC's own version
/// and never had to match `PMID@Version`, and three affiliation
/// identifier sources and one language code name nothing and are dropped.
/// Only the first group changes the layout; the rest change which records
/// exist. Normalized output under version 4 is rebuilt, not patched,
/// because it is both misdescribed and short of those records.
///
/// Version 6 (2026-09-24): dates. The survey of the whole 2026 baseline
/// on version 5 found 10,164 records it could not read, all but 106 of
/// them on a date (`docs/inbox/date-policy.md`). Most were shapes, not
/// false values: `MedlineDate` seasons in capitals (`1976 WINTER`),
/// `1992 Aug 15-Sep`, a year written again after the months
/// (`2016 Nov-Dec 2016`) or after the first of them
/// (`2017 Dec 2016 - Jan`), and a four-digit number where a day goes
/// now reports as the shape it is rather than as a day out of range. Two
/// changes read what was already accepted differently: a range whose end
/// month comes before its start month, with no second year written, ends
/// the year after (`1990 Dec 21-Jan 10` ends in 1991, not in the January
/// before it), and a range that still ends before it starts is refused
/// (`1990 Dec 12-8`), where it used to be kept, wrong. And a PubMed
/// history date or time that names none — `2011-09-31`, `null:null` — is
/// kept as written beside why, and its record normalizes (the user's
/// decision, 2026-09-24), which changes the layout. Normalized output
/// under version 5 is rebuilt, not patched.
///
/// Version 7 (2026-09-24): the survey on version 6 found 9,503 records it
/// could not read, every one a `MedlineDate`. Two shapes are read: a
/// month of a volume over two years (`1992-1993 Oct`,
/// [`date::VolumeMonth`]) and a seasonal volume written with both years
/// (`1986 Winter-1987 Spring`). The rest are kept rather than read: a
/// `MedlineDate` the grammar cannot read whole, that begins with a year,
/// keeps that year — or, for a day range within a month whose end comes
/// before its start (`1995 Mar 22-8`), its first day — beside the text as
/// written and why, and its record normalizes; one that does not begin
/// with a year still stops the run (the user's decision, 2026-09-24).
/// Which of two readings such a day range means is not in the record, so
/// its end is not kept. PubMed's `Dates.published` became a
/// `normalize::Published`, which changes the layout. Normalized output
/// under version 6 is rebuilt, not patched.
pub const VERSION: u32 = 7;

pub mod claim;
pub mod code;
pub mod date;
pub mod doi;
mod human;
pub mod id;
pub mod isbn;
pub mod isni;
pub mod issn;
pub mod lang;
pub mod mesh;
pub mod orcid;
pub mod ror;
pub mod symbol;

pub use claim::Claim;
pub use code::{CountryCode, CurrencyCode, Error as CodeError};
pub use date::{
    Date, DateTime, Day, Error as DateError, MedlineError as MedlineDateError, Month, PartialDate,
    Quarter, Season, Seasons, TimeOfDay, VolumeMonth, Year,
};
pub use doi::{Doi, Error as DoiError, Registrant};
pub use id::{OpenAlexId, ParseError, PmcId, Pmid};
pub use isbn::{Error as IsbnError, Isbn};
pub use isni::{Error as IsniError, Isni};
pub use issn::{Error as IssnError, Issn};
pub use lang::{Error as LangError, Lang};
pub use mesh::{Error as MeshUiError, MeshUi};
pub use orcid::{Error as OrcidError, Orcid};
pub use ror::{Error as RorError, Ror};
pub use symbol::{Interned, Interner, Table};
