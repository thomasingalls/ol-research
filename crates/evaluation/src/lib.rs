//! Measuring search (`docs/evaluation-plan.md`).
//!
//! A run — what the search returned for every query of a suite, in rank
//! order — and an answer key are files in TREC's formats ([`trec`]); the
//! measures are computed per query from them ([`measure`]) exactly as
//! NIST's `trec_eval` computes them, and every average carries a 95%
//! interval from resampling the queries ([`interval`]).

pub mod interval;
pub mod measure;
pub mod report;
pub mod trec;
