//! Measuring search (`docs/evaluation-plan.md`).
//!
//! A run — what the search returned for every query of a suite, in rank
//! order — and an answer key are files in TREC's formats ([`trec`]); the
//! measures are computed per query from them ([`measure`]) exactly as
//! NIST's `trec_eval` computes them, and every average carries a 95%
//! interval from resampling the queries ([`interval`]).
//!
//! For trying many changes cheaply (§14): a sample of a suite's working
//! half ([`suite::Suite::sample`]), the works a small index is built over
//! ([`screening`]), and whether two runs are one answer ([`same`]).

pub mod interval;
pub mod measure;
pub mod report;
pub mod same;
pub mod screening;
pub mod sets;
pub mod suite;
pub mod trec;
