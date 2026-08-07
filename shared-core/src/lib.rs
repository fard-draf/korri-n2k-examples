#![cfg_attr(not(feature = "std"), no_std)]
#![allow(async_fn_in_trait)]

/// Runtime free: the ISO identities and the PGN generators. Both runtimes use them.
pub mod instances;
pub mod samples;

/// The publishing loop, one version per runtime.
pub mod publisher;

/// Actisense and NGT-1 framing, for a board that acts as a gateway.
/// No example uses it yet.
#[cfg(feature = "embassy")]
pub mod format;
