//! The transport (serving plan §7): the query core over gRPC. Public ids
//! on the wire and nothing else — a request names a work by its public
//! id or by any name, and every answer speaks public ids, the boundary
//! where `WorkId`s are mapped both ways. Every answer says which epoch
//! it came from and under whose terms. [`service`] is the server over
//! anything that [`answers::Answers`], which the serving layer's core
//! does and a test fake can; [`terms`] is the stub of the terms module.

pub mod answers;
pub mod convert;
pub mod service;
pub mod terms;

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
pub use service::Service;
pub use terms::Terms;
