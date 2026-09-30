//! Citation lookup (docs/citation-lookup-plan.md, 2026-09-27): a person
//! pastes one citation — a line from a reference list — and is told which
//! works it may be, and why.
//!
//! This crate is the part that needs no index and no store: reading a
//! citation. The server supplies what it holds — the works a name names,
//! the candidates a search turns up — and asks these functions what the
//! citation says about them. For one citation at a time, and nothing in
//! ingestion (the user, 2026-09-27); shaped so that a batch could call the
//! same functions later, and no more than that.
//!
//! - [`identifiers`]: the DOIs, PMIDs and PMC ids written in a citation
//!   (the plan's CL1).
//! - [`text`]: text as the check compares it — Unicode's canonical
//!   caseless match, and nothing looser yet (D11).
//! - [`check`]: whether a citation says what a work says, field by field,
//!   all of them or it fails (CL2, D4).

pub mod check;
pub mod identifiers;
pub mod text;
