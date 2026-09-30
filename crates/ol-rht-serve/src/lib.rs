//! The transport (serving plan §7): the query core over gRPC and, for a
//! browser, the same answers over JSON on `axum`. Public ids
//! on the wire and nothing else — a request names a work by its public
//! id or by any name, and every answer speaks public ids, the boundary
//! where `WorkId`s are mapped both ways. Every answer says which epoch
//! it came from and under whose terms. [`service`] is the server over
//! anything that [`answers::Answers`], which the serving layer's core
//! does and a test fake can; [`http`] is the same service over JSON, for
//! a browser; [`terms`] is the stub of the terms module.

pub mod answers;
pub mod convert;
pub mod http;
/// A citation looked up: what it names, what its words turn up, each work
/// checked against it.
mod lookup;
/// The query string a route reads its parameters from, as the struct
/// the route declares: one list of names instead of three.
mod query;
/// One line per search, apart from the fault log.
mod querylog;
pub mod service;
pub mod terms;
/// The browser client's TypeScript, from these same types. Tests only:
/// it generates `web/src/generated/serving.ts` and fails when what is
/// committed there is not what the protocol generates today.
#[cfg(test)]
mod ts;

/// The generated protocol.
#[allow(
    clippy::pedantic,
    clippy::all,
    missing_docs,
    unreachable_pub,
    unused_qualifications,
    missing_debug_implementations
)]
pub mod proto {
    /// `olrht.v1`.
    pub mod v1 {
        tonic::include_proto!("olrht.v1");
    }

    /// The descriptor set, for reflection.
    pub const DESCRIPTORS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/olrht.bin"));
}

pub use answers::Answers;
pub use http::{Origins, router};
pub use service::Service;
pub use terms::Terms;
