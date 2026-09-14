//! Shared ingestion plumbing (plan §3.3). Today: reading raw files. Later:
//! the `Source` / `Parser` / `Normalizer` / `Sink` traits, the runner, fetch
//! and manifests — added when a second source makes the common shape
//! evident rather than guessed.

pub mod gz;
