//! The merged layer (merge plan §1): one work across sources, built from
//! the normalized layer and nothing else. Identity — which records are one
//! work — lives here because it needs every record at once, which is
//! exactly what ingestion never needs.
//!
//! The vocabulary is the plan's §2. A [`WorkId`] is ours: minted by the
//! [`registry`], dense, opaque, derived from nothing. A [`Name`] is what
//! the world calls a work — a DOI, a PMID, a PMC id, an OpenAlex id — and
//! a work has many, each with who asserted it and a verdict. A *record* is
//! one source's normalized record of the work; a *stub* is a work with
//! names and no records, cited by something we hold.
//!
//! Nothing here knows a source. Each source crate implements
//! [`Contributor`] for its own records — the names a work asserts, its
//! titles, the names a reference points at — and the binary registers
//! each with one line, the way it does for [`ingest::Source`].
//!
//! [`identity`] is the first pass of the plan's §8: every work's
//! assertions, the components they connect, the verdicts of §3.3, and the
//! works those leave. [`registry`] is the plan's §4: the one part of the
//! merged layer that is of record. [`build`] is the second pass: every
//! record's [`view`] joins its work, every reference resolves through
//! the names, and the rows of [`record`] are made — the merged record
//! with its promoted fields (§5), the edges, relations and unresolved
//! references (§6) — which [`layout`] writes as sharded sets. [`delta`] is
//! what an incremental merge appends beside them (serving plan §5), and
//! [`journal`] the clock every run advances.

pub mod arena;
pub mod build;
pub mod contributor;
pub mod copies;
pub mod delta;
pub mod dictionary;
pub mod document;
pub mod identity;
pub mod journal;
pub mod kind;
pub mod layout;
pub mod name;
pub mod placement;
pub mod public;
pub mod publisher;
pub mod record;
pub mod registry;
pub mod relation;
pub mod report;
pub mod runs;
pub mod sets;
pub mod spill;
pub mod title;
pub mod view;
pub mod work;

pub use build::{Builder, Ids, Merged, OwnedWork};
pub use contributor::Contributor;
pub use document::{Assembler, Assembly, Document, Documents};
pub use identity::Identity;
pub use kind::{Form, Genre};
pub use name::{ALIASES, Assertion, Name, OwnedName};
pub use registry::Registry;
pub use title::Titles;
pub use view::View;
pub use work::{SourceId, Sources, WorkId};

/// The merge's own version, beside the model's: any change to the rules
/// of the plan's §3 or §5 or to the shapes written under `merged/` bumps
/// it, and merged sets under another version are rebuilt, never patched.
/// Version 2 (2026-09-16): every record in a merged row carries its
/// assertions and its view, the evidence the incremental merge re-runs
/// the rules on, and a row's lists are in canonical order. Version 3
/// (2026-09-17): the newest copy of a record speaks for all of it, its
/// view names its set and version, and a stub name's sources are its
/// citers'. Version 4 (2026-09-18): the newest copy still speaks for
/// names, fields, relations and text, but a record's citations are the
/// union over every copy a deletion has not withdrawn — PubMed replaces a
/// publisher's reference list with PMC's when an article lands there, and
/// the identifiers the publisher gave are evidence we already hold.
/// Version 5 (2026-09-18): absence is not an assertion. A record's names
/// are the union over its standing copies, and each promoted field is
/// the newest copy's value or, where the newest has none, the newest
/// copy that has one; its text follows the same rule. Deletions,
/// relations and a value that is merely shorter are unchanged.
/// Version 6 (2026-09-18): a reference whose names name different works
/// is settled by its own citation text where that text repeats one of
/// its DOIs, the order of §6 deciding only where nothing corroborates.
/// Version 7 (2026-09-19): a stub's name carries, with each citer,
/// whether that reference's own citation text repeats the name — what
/// lets a reader settle the reference again, by §6 and not by an
/// imitation of it, when a record takes over part of the stub.
/// Version 8 (2026-09-19): publisher ids are names. A publisher's own
/// identifier for an article — PubMed's two `pii` slots, Crossref's
/// non-DOI `alternative-id` — is a name in a namespace of its own,
/// `reg1016:S0006349525001234`, keyed by the registrant of the record's
/// DOI and written exactly as the publisher wrote it but for Elsevier's
/// printed PII, which is written bare ([`publisher`]). Every merged row,
/// the registry and the delta logs gain names of that namespace, and
/// extending `publisher::ELSEVIER` changes the canonical spelling of
/// names already written, so that too is a full merge. With them, a
/// rejected assertion carries which rule rejected it
/// ([`identity::Reason`]) and a holder only where a record holds the
/// name, because a publisher id one source claims for more than one of
/// its own records is no article's identifier and no record holds it.
/// Version 9 (2026-09-19): a `cited_as` row says whether the reference
/// also named a held work *other than* the one it landed on. Item 14
/// makes no stub name of such a reference's names, and until now only
/// the pass that read the reference knew — a batch dividing that work
/// later had to assume it named none, and minted stubs a full merge
/// does not ([`record::CitedAs::beside_held`], serving plan §11, item
/// 26). The mark is written by the second pass, so it is a full merge.
/// Version 10 (2026-09-23): a record's status says whether PubMed marked
/// it a duplicate publication ([`view::Status::duplicate`]), beside
/// retracted, corrected and concerned. The type says a record duplicates
/// another without saying which — PubMed has no `Duplicate` relation, so
/// unlike a retraction there is nothing to follow — and the merge does
/// not pair them; it carries the mark so a later pass can. Only the
/// shape under `merged/` changes, not a rule, and the model's own
/// version moves with it, but the shape is what this number is for.
/// Version 11 (2026-09-24): a container's DOI on its parts is the
/// container's. When a component beyond the rules is a star — one name,
/// or a few, carried by every record of it, every other name held by one
/// record, and no two of its titles agreeing — the shared names are the
/// DOI of the supplement or proceedings the records were printed in, not
/// of any one of them (`container-id`, since gone). Each part keeps
/// its own names and is refused the container's, with the container's
/// own record as the holder where one is read; a reference naming it
/// lands on that record, or on a stub for the container where none is.
/// The parts and the container are not yet linked: merge plan §11 item
/// 25 is how they will be.
/// Version 12 (2026-09-24): version 11's rule, named for what it knows.
/// The corpus survey showed a star is not always a container's —
/// `10.1016/s0197` on 38 records from two journals is a DOI cut short —
/// so the shared names come off with the verdicts the rules already have:
/// no article's id ([`identity::Reason::NotAnArticleId`]) where no record
/// holds the name, and a wrong link where one does; `container-id` is
/// gone. And two records are copies of one article, which leaves the star
/// a hairball, only if their titles are equal or one is the other's
/// vernacular: §3.3's prefix clause called two letters answering one
/// viewpoint copies. No set was ever written under version 11.
/// Version 13 (2026-09-24): a publisher id with markup in its value —
/// `IdType="pii">JEU342`, an attribute leaked in front of the id in NLM's
/// own file — is a malformed claim and no name
/// ([`publisher::Refusal::Markup`]); on the 2026 baseline 62 distinct
/// values the shape rules let through, by the survey's dictionary before
/// and after. A name of that shape already in a registry still reads, and
/// a complete merge drops it.
/// Version 14 (2026-09-25): a rejected assertion says which record made
/// it and the field it came from ([`record::Rejected::record`]). A claim
/// refused as no article's id, or cut off a star, is on no record's list
/// of assertions, and whether it is refused is a fact about every record
/// claiming it, so an incremental merge has to find the other claimants
/// and hand the claim back to the identity pass to decide it as the full
/// merge does (merge plan §11, item 24). And a record's assertions are
/// in canonical order, its native name first and then by name, rather
/// than the order its copies were read in: a batch reads a record's
/// newest copy first and the full merge its oldest, and the update-files
/// harness found 28 records whose names differed only in order.
/// Version 15 (2026-09-25): a reference is counted once however many of a
/// record's copies carry it — the user's ruling — where the full merge
/// counted it once a copy: an arc two copies repeat was no one
/// reference's (`sole` false in its cited-as rows, so it never moved
/// whole) and its unresolved row was written twice. Two references are
/// one when their names and their citation, whole, are equal
/// ([`build::fingerprint`]); the same holds for a source's two records of
/// one work, whose references land from the same work. With it, what an
/// incremental merge needs to restate a record's evidence as the full
/// merge writes it (serving plan §11, item 35): a record's view names
/// the sets of the copies its speaking copy supersedes
/// ([`view::View::superseded`]), so a batch can read every copy back; a
/// relation keeps its cited-as row whatever it names, a stub included,
/// so a batch that moves the name finds who relates to it; and a
/// citation is read as text, a container given as a symbol of its set's
/// table resolved, so that an unresolved row keeps its container and the
/// same reference in two sets prints alike.
///
/// Version 16 (2026-09-27): Crossref's `aliases` are held back from the
/// identity pass's joins until every record is in, as publisher ids are,
/// and three rules decide which name the record's work (merge plan §11
/// item 28, the user's rules; the survey of Crossref's whole March 2026
/// file stopped on one component of each shape). An alias several
/// records of one source give, and none holds as its own DOI, names none
/// of them ([`identity::Reason::NotAnArticleId`]): a volume's DOI, cut
/// short, that 36 articles each give. An alias a record of the
/// claimant's own work says it corrects, retracts or updates is not the
/// work's name ([`identity::Reason::Corrected`]): a correction whose
/// deposit gives the 102 abstracts it corrects as its aliases. And a
/// container's aliases — a journal's, a volume's, an issue's, a series' —
/// are not its names ([`identity::Reason::Container`]): a journal's
/// record giving 655 of its articles' old DOIs. A name those rules take
/// off every record that gave it is in no work, and a stub's where cited.
/// Every other alias joins as before. Amended 2026-09-28, before anything
/// was merged of record under it: a source may list deposits whose
/// aliases are what they correct ([`Contributor::corrects_its_aliases`]),
/// which rule 2 then reads as if a record said so. Crossref's list holds
/// that correction alone, since PubMed's 2026 baseline predates its
/// erratum's links to the abstracts (merge plan §13, a compromise).
/// Amended again the same day: a publisher id's two PII spellings are one
/// name under every registrant, the shape deciding ([`publisher`]), where
/// a list of Elsevier's registrants used to. The survey of Crossref's
/// whole file split the two records of one article where PubMed's
/// punctuated PII and Crossref's bare one, under a registrant off the
/// list, were two names.
///
/// Version 17 (2026-09-30): where a work was published (the citation
/// fields plan's CF2, on model 13). Each record's view carries its
/// placement ([`placement`]): the journal by its ids, which was the view's
/// venue, the levels below it as its volume and issue read, where the
/// work sits (its pages or article number), and the issue's date, the
/// one a citation prints; and its online date. The promoted fields join
/// the records' placements by the user's rules of 2026-09-30: compared
/// level by level and by first page, joined where they agree but for
/// notation, a level only one gives added; where any differs, Crossref's
/// whole placement stands and the others' values are its disagreements;
/// joins with nothing shared counted apart. `venue` as a promoted field of
/// its own is gone: the journal is the placement's top level, its ISSNs
/// those of every record that agrees. Authors go Crossref, PubMed,
/// OpenAlex (D5). The rows' layout changes, and a merge under version 16
/// is rebuilt, not patched.
pub const VERSION: u32 = 17;
