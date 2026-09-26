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
///
/// Version 8 (2026-09-24): a day range within one month whose end has
/// fewer digits than its start, and reads as before it, takes the start's
/// leading digits — `1995 Mar 22-8` is the 22nd to the 28th, NLM's
/// notation, as `14-5` is pages 14 to 15 on the same records (the user's
/// call). The count over the whole baseline found 16 such issues, 242
/// records; every one with a next issue in its volume is followed before a
/// next-month end could fall, and no range is written so that only the
/// next month would fit. Those records were kept with their first day
/// under version 7 and are read now. So is a season written before its
/// year (`Spring 2008`), which the survey on version 7 found on 3,044
/// records of 2006 to 2017 that still stopped the run, because they do not
/// begin with a year. Both change what normalized output holds. Output
/// under version 7 is rebuilt, not patched.
///
/// Version 9 (2026-09-25): U+2010 HYPHEN in a DOI's suffix is read as `-`
/// ([`Doi`], the user's decision). Crossref's March 2026 sample carries it
/// in 13,305 of its 262,499 reference DOIs, every one from Wiley, and the
/// merge had been dropping those references without a trace, since a
/// reference whose one name is malformed has nothing to land on. A DOI
/// that was malformed under version 8 is a name now, wherever a source
/// writes it, which changes what normalized output holds. Output under
/// version 8 is rebuilt, not patched.
///
/// Version 10 (2026-09-26): what the survey of Crossref's whole March 2026
/// file found (ingestion plan §10). Institutions: an author's affiliation
/// and a work's own `institution[]` — one element of the deposit schema —
/// normalize alike, keeping a name, acronyms, places and departments as
/// written, and a ROR (a claim: one record's fails its checksum), an ISNI
/// and a Wikidata item ([`Qid`]), each with who asserted it; 1,957 shards
/// stopped at a Wikidata id and 892 at an affiliation's acronym. The
/// work's own institutions were dropped before. Types: a Crossref record
/// may have no `type`, which is unknown rather than guessed (2,243
/// shards); an update keeps the word it was written in beside what it is
/// read as, Crossmark's free-text years having left `err`,
/// `corrected-article`, `Erratum` and others in the data (about 600
/// shards); and an update can be a comment, which Crossmark's twelve lack.
/// Dates: a license's start may be absent (3,272 shards), and is read as
/// from publication, as a day, or as written and unreadable, its delay
/// checked and dropped; and the dates beside the publication dates —
/// approved, posted, accepted, published-other, a dataset's content
/// created and updated (9,641 shards) — are kept with what each is.
/// Fields: a work's version label and its part of a multi-volume set are
/// kept as written, and posted content's `status` — withdrawn or removed,
/// when, and why. After the survey of the whole file on version 10, an
/// empty affiliation is set aside rather than failing, as is an author
/// with nothing in it (303,155 records), a resource link may be `null`
/// or the resource absent, a journal issue may hold only its date,
/// a record may name no member and no publisher (a DOI moved to an owner
/// Crossref's index does not attach), an ISNI, an ISBN and an author's
/// ORCID are claims, and a language may be a three-letter code. DOIs, in
/// every source: a letter beyond ASCII is the DOI of record, lowercased;
/// every Unicode dash is read as `-`, whitespace at an end dropped, and a
/// letter written as its UTF-8 bytes read as Latin-1 repaired — each
/// reading counted ([`doi::Readings`]); a Crossref record keeps its
/// `aliases`, other DOIs the registry holds for the work, and its DOI's
/// ASCII spelling ([`Doi::ascii_spelling`]), a name of the work too, so a
/// publisher's ASCII twin of an accented DOI is the same work. Nothing
/// was ever normalized under the version 10 builds before
/// those (only surveyed), so they are version 10 too. Output under version
/// 9 is rebuilt, not patched.
pub const VERSION: u32 = 10;

pub mod claim;
pub mod code;
pub mod date;
pub mod doi;
mod doi_ascii;
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
pub use id::{OpenAlexId, ParseError, PmcId, Pmid, Qid};
pub use isbn::{Error as IsbnError, Isbn};
pub use isni::{Error as IsniError, Isni};
pub use issn::{Error as IssnError, Issn};
pub use lang::{Error as LangError, Lang};
pub use mesh::{Error as MeshUiError, MeshUi};
pub use orcid::{Error as OrcidError, Orcid};
pub use ror::{Error as RorError, Ror};
pub use symbol::{Interned, Interner, Table};
